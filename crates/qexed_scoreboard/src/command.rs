//! /scoreboard 指令节点：objectives 与 players 两大子命令族。
//!
//! 架构：executor 把 [ScoreboardOp] 提交进共享队列，宿主（qexed_play）在
//! dispatch 成功后 drain 队列，用 [apply_op] 应用到权威状态并发包/回显。
//! 指令树保持无状态可静态声明，MutexGuard 不进 executor、不跨 await。

use std::sync::{Arc, Mutex};

use qexed_command::{CommandContext, CommandError, CommandNode, argument, literal};
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::commands::CommandArgumentType as Parser;

/// /scoreboard 执行产生的动作，宿主负责应用到状态机并生成回显/同步包。
#[derive(Debug, Clone, PartialEq)]
pub enum ScoreboardOp {
    /// objectives add <name> <criteria> [display...]
    AddObjective { name: String, criteria: String, display: String },
    /// objectives remove <name>
    RemoveObjective { name: String },
    /// objectives list
    ListObjectives,
    /// objectives setdisplay <slot> [objective]
    SetDisplay { slot: String, objective: Option<String> },
    /// objectives modify <name> displayname <display...>
    ModifyDisplayName { name: String, display: String },
    /// players list [holder]
    ListScores { holder: Option<String> },
    /// players set <holder> <objective> <score>
    SetScore { holder: String, objective: String, score: i32 },
    /// players get <holder> <objective>
    GetScore { holder: String, objective: String },
    /// players add <holder> <objective> <delta>
    AddScore { holder: String, objective: String, delta: i32 },
    /// players reset <holder> [objective]
    ResetScore { holder: String, objective: Option<String> },
}

/// apply_op 的产物：回显、同步包或拒绝原因。
#[derive(Debug, Clone, PartialEq)]
pub enum ScoreboardEffect {
    /// 应用成功后发给玩家的系统聊天行。
    Reply(String),
    /// 应用成功后需要下发的客户端同步包。
    Packet(crate::SyncPacket),
    /// 状态机拒绝（目标不存在 / 重名 / 槽位非法），转错误回显。
    Rejected(String),
}

/// op 共享队列：executor 收集，宿主 drain 后逐个 apply。
pub type OpQueue = Arc<Mutex<Vec<ScoreboardOp>>>;

/// 便捷：向队列提交一个 op。
fn submit(ops: &OpQueue, op: ScoreboardOp) {
    ops.lock().unwrap().push(op);
}

/// 把一个 op 应用到权威状态机，产出回显与同步包序列。
pub fn apply_op(sb: &mut crate::Scoreboard, op: ScoreboardOp) -> Vec<ScoreboardEffect> {
    let mut out = Vec::new();
    match op {
        ScoreboardOp::AddObjective { name, criteria, display } => {
            if sb.has_objective(&name) {
                out.push(ScoreboardEffect::Rejected(format!(
                    "An objective named {name} already exists"
                )));
            } else if sb.add_objective(&name, &criteria, crate::text_component(&display)).is_some() {
                out.push(ScoreboardEffect::Packet(crate::SyncPacket::Objective(
                    crate::SetObjective::create(name.clone(), crate::text_component(&display)),
                )));
                out.push(ScoreboardEffect::Reply(format!(
                    "Added new scoreboard objective [{name}] successfully"
                )));
            } else {
                out.push(ScoreboardEffect::Rejected("Objective name cannot be empty".into()));
            }
        }
        ScoreboardOp::RemoveObjective { name } => {
            if sb.has_objective(&name) {
                sb.remove_objective(&name);
                out.push(ScoreboardEffect::Packet(crate::SyncPacket::Objective(
                    crate::SetObjective::remove(name.clone()),
                )));
                out.push(ScoreboardEffect::Reply(format!(
                    "Removed objective [{name}] successfully"
                )));
            } else {
                out.push(ScoreboardEffect::Rejected(format!(
                    "No objective was found with name [{name}]"
                )));
            }
        }
        ScoreboardOp::ListObjectives => {
            let objectives: Vec<String> = sb
                .objectives()
                .map(|o| format!("[{}] ({})", o.name, o.criteria))
                .collect();
            out.push(ScoreboardEffect::Reply(if objectives.is_empty() {
                "There are no objectives".to_string()
            } else {
                format!(
                    "There are {} objectives: {}",
                    objectives.len(),
                    objectives.join(", ")
                )
            }));
        }
        ScoreboardOp::SetDisplay { slot, objective } => {
            let Some(slot_id) = parse_slot(&slot) else {
                out.push(ScoreboardEffect::Rejected(format!("No slot was found named {slot}")));
                return out;
            };
            match objective.as_deref() {
                Some(obj) => {
                    if !sb.has_objective(obj) {
                        out.push(ScoreboardEffect::Rejected(format!(
                            "Unknown scoreboard objective [{obj}]"
                        )));
                        return out;
                    }
                    sb.set_display(slot_id, Some(obj));
                    out.push(ScoreboardEffect::Packet(crate::SyncPacket::Display(
                        crate::SetDisplayObjective {
                            slot: VarInt(slot_id),
                            objective_name: obj.to_string(),
                        },
                    )));
                    out.push(ScoreboardEffect::Reply(format!("Set slot display to [{obj}]")));
                }
                None => {
                    sb.set_display(slot_id, None);
                    out.push(ScoreboardEffect::Packet(crate::SyncPacket::Display(
                        crate::SetDisplayObjective {
                            slot: VarInt(slot_id),
                            objective_name: String::new(),
                        },
                    )));
                    out.push(ScoreboardEffect::Reply("Cleared slot display".into()));
                }
            }
        }
        ScoreboardOp::ModifyDisplayName { name, display } => {
            if !sb.modify_display_name(&name, crate::text_component(&display)) {
                out.push(ScoreboardEffect::Rejected(format!(
                    "Unknown scoreboard objective [{name}]"
                )));
                return out;
            }
            out.push(ScoreboardEffect::Packet(crate::SyncPacket::Objective(
                crate::SetObjective::update(name.clone(), crate::text_component(&display)),
            )));
            out.push(ScoreboardEffect::Reply(format!("Changed the display name of [{name}]")));
        }
        ScoreboardOp::ListScores { holder } => match holder {
            Some(holder) => {
                let rows = sb.holder_entries(&holder);
                out.push(ScoreboardEffect::Reply(if rows.is_empty() {
                    format!("{holder} has no scores to show")
                } else {
                    let body: Vec<String> = rows
                        .iter()
                        .map(|(obj, score)| format!("[{obj}]: {score}"))
                        .collect();
                    format!("{} show {} scores: {}", holder, rows.len(), body.join(", "))
                }));
            }
            None => {
                let holders = sb.holders();
                out.push(ScoreboardEffect::Reply(if holders.is_empty() {
                    "There are no tracked entities".to_string()
                } else {
                    format!(
                        "There are {} tracked entities: {}",
                        holders.len(),
                        holders.join(", ")
                    )
                }));
            }
        },
        ScoreboardOp::SetScore { holder, objective, score } => {
            if !sb.has_objective(&objective) {
                out.push(ScoreboardEffect::Rejected(format!(
                    "Unknown scoreboard objective [{objective}]"
                )));
                return out;
            }
            sb.set_score(&objective, &holder, score);
            out.push(ScoreboardEffect::Packet(crate::SyncPacket::Score(
                crate::SetScore::new(holder.clone(), objective.clone(), score, None),
            )));
            out.push(ScoreboardEffect::Reply(format!(
                "Set [{holder}] for objective [{objective}] to {score}"
            )));
        }
        ScoreboardOp::GetScore { holder, objective } => {
            if !sb.has_objective(&objective) {
                out.push(ScoreboardEffect::Rejected(format!(
                    "Unknown scoreboard objective [{objective}]"
                )));
                return out;
            }
            match sb.get_score(&objective, &holder) {
                Some(score) => out.push(ScoreboardEffect::Reply(format!(
                    "[{objective}] for [{holder}] = {score}"
                ))),
                None => out.push(ScoreboardEffect::Rejected(format!(
                    "No score for [{holder}] in [{objective}]"
                ))),
            }
        }
        ScoreboardOp::AddScore { holder, objective, delta } => {
            match sb.add_score(&objective, &holder, delta) {
                Some(new_score) => {
                    out.push(ScoreboardEffect::Packet(crate::SyncPacket::Score(
                        crate::SetScore::new(holder.clone(), objective.clone(), new_score, None),
                    )));
                    out.push(ScoreboardEffect::Reply(format!(
                        "Set [{holder}] to {new_score} (delta {delta:+})"
                    )));
                }
                None => out.push(ScoreboardEffect::Rejected(format!(
                    "Unknown scoreboard objective [{objective}]"
                ))),
            }
        }
        ScoreboardOp::ResetScore { holder, objective } => {
            if sb.holder_entries(&holder).is_empty() {
                out.push(ScoreboardEffect::Rejected(format!("No scores for [{holder}]")));
                return out;
            }
            match objective.as_deref() {
                Some(obj) => {
                    sb.reset_score(obj, &holder);
                    out.push(ScoreboardEffect::Packet(crate::SyncPacket::Reset(
                        crate::ResetScore {
                            owner: holder.clone(),
                            objective_name: Some(obj.to_string()),
                        },
                    )));
                }
                None => {
                    sb.reset_holder(&holder, None);
                    out.push(ScoreboardEffect::Packet(crate::SyncPacket::Reset(
                        crate::ResetScore {
                            owner: holder.clone(),
                            objective_name: None,
                        },
                    )));
                }
            }
            out.push(ScoreboardEffect::Reply(format!("Reset scores for [{holder}]")));
        }
    }
    out
}

/// 槽位名 → 协议编号。原版 list/sidebar/belowName + 常用别名。
fn parse_slot(name: &str) -> Option<i32> {
    match name {
        "list" => Some(crate::slot::LIST),
        "sidebar" => Some(crate::slot::SIDEBAR),
        "belowName" | "below_name" | "belowname" => Some(crate::slot::BELOW_NAME),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 指令树声明
// ---------------------------------------------------------------------------

fn integer(ctx: &CommandContext, name: &str) -> Result<i32, CommandError> {
    ctx.args.int(name).ok_or_else(|| CommandError::BadArgument {
        name: name.to_string(),
        expected: "i32",
        raw: ctx.args.text(name).unwrap_or("").to_string(),
    })
}

fn text<'a>(ctx: &'a CommandContext, name: &str) -> Result<&'a str, CommandError> {
    ctx.args
        .text(name)
        .ok_or_else(|| CommandError::InvalidUsage { usage: name.to_string() })
}

/// 组装 /scoreboard 子树（含全部 executor），由宿主挂到自己的指令树。
pub fn scoreboard_node(ops: OpQueue) -> CommandNode {
    literal("scoreboard")
        .child(objectives_branch(&ops))
        .child(players_branch(ops))
}

fn objectives_branch(ops: &OpQueue) -> CommandNode {
    literal("objectives")
        // /scoreboard objectives —— 列出全部目标
        .executes({
            let ops = ops.clone();
            move |_| {
                submit(&ops, ScoreboardOp::ListObjectives);
                Ok(())
            }
        })
        // objectives add <name> <criteria> [display...]
        .child(
            literal("add").child(
                argument("name", Parser::MinecraftObjective, false).child(
                    argument("criteria", Parser::MinecraftObjectiveCriteria, false)
                        .executes({
                            let ops = ops.clone();
                            move |ctx| {
                                submit(&ops, ScoreboardOp::AddObjective {
                                    name: text(ctx, "name")?.to_string(),
                                    criteria: text(ctx, "criteria")?.to_string(),
                                    display: text(ctx, "name")?.to_string(),
                                });
                                Ok(())
                            }
                        })
                        .child(
                            argument("display", Parser::BrigadierString(Default::default()), true)
                                .executes({
                                    let ops = ops.clone();
                                    move |ctx| {
                                        submit(&ops, ScoreboardOp::AddObjective {
                                            name: text(ctx, "name")?.to_string(),
                                            criteria: text(ctx, "criteria")?.to_string(),
                                            display: text(ctx, "display")?.to_string(),
                                        });
                                        Ok(())
                                    }
                                }),
                        ),
                ),
            ),
        )
        // objectives remove <name>
        .child(
            literal("remove").child(
                argument("name", Parser::MinecraftObjective, false).executes({
                    let ops = ops.clone();
                    move |ctx| {
                        submit(&ops, ScoreboardOp::RemoveObjective {
                            name: text(ctx, "name")?.to_string(),
                        });
                        Ok(())
                    }
                }),
            ),
        )
        // objectives list
        .child(
            literal("list").executes({
                let ops = ops.clone();
                move |_| {
                    submit(&ops, ScoreboardOp::ListObjectives);
                    Ok(())
                }
            }),
        )
        // objectives setdisplay <slot> [objective]
        .child(
            literal("setdisplay").child(
                argument("slot", Parser::MinecraftScoreboardSlot, false)
                    .executes({
                        let ops = ops.clone();
                        move |ctx| {
                            submit(&ops, ScoreboardOp::SetDisplay {
                                slot: text(ctx, "slot")?.to_string(),
                                objective: None,
                            });
                            Ok(())
                        }
                    })
                    .child(
                        argument("objective", Parser::MinecraftObjective, false).executes({
                            let ops = ops.clone();
                            move |ctx| {
                                submit(&ops, ScoreboardOp::SetDisplay {
                                    slot: text(ctx, "slot")?.to_string(),
                                    objective: Some(text(ctx, "objective")?.to_string()),
                                });
                                Ok(())
                            }
                        }),
                    ),
            ),
        )
        // objectives modify <name> displayname <display...>
        .child(
            literal("modify").child(
                argument("name", Parser::MinecraftObjective, false).child(
                    literal("displayname").child(
                        argument("display", Parser::BrigadierString(Default::default()), true)
                            .executes({
                                let ops = ops.clone();
                                move |ctx| {
                                    submit(&ops, ScoreboardOp::ModifyDisplayName {
                                        name: text(ctx, "name")?.to_string(),
                                        display: text(ctx, "display")?.to_string(),
                                    });
                                    Ok(())
                                }
                            }),
                    ),
                ),
            ),
        )
}

fn players_branch(ops: OpQueue) -> CommandNode {
    literal("players")
        // players list [holder]
        .child(
            literal("list")
                .executes({
                    let ops = ops.clone();
                    move |_| {
                        submit(&ops, ScoreboardOp::ListScores { holder: None });
                        Ok(())
                    }
                })
                .child(
                    argument("holder", Parser::MinecraftScoreHolder(Default::default()), false)
                        .executes({
                            let ops = ops.clone();
                            move |ctx| {
                                submit(&ops, ScoreboardOp::ListScores {
                                    holder: Some(text(ctx, "holder")?.to_string()),
                                });
                                Ok(())
                            }
                        }),
                ),
        )
        // players set <holder> <objective> <score>
        .child(
            literal("set").child(
                argument("holder", Parser::MinecraftScoreHolder(Default::default()), false)
                    .child(
                        argument("objective", Parser::MinecraftObjective, false).child(
                            argument("score", Parser::BrigadierInteger(Default::default()), false)
                                .executes({
                                    let ops = ops.clone();
                                    move |ctx| {
                                        submit(&ops, ScoreboardOp::SetScore {
                                            holder: text(ctx, "holder")?.to_string(),
                                            objective: text(ctx, "objective")?.to_string(),
                                            score: integer(ctx, "score")?,
                                        });
                                        Ok(())
                                    }
                                }),
                        ),
                    ),
            ),
        )
        // players get <holder> <objective>
        .child(
            literal("get").child(
                argument("holder", Parser::MinecraftScoreHolder(Default::default()), false)
                    .child(
                        argument("objective", Parser::MinecraftObjective, false).executes({
                            let ops = ops.clone();
                            move |ctx| {
                                submit(&ops, ScoreboardOp::GetScore {
                                    holder: text(ctx, "holder")?.to_string(),
                                    objective: text(ctx, "objective")?.to_string(),
                                });
                                Ok(())
                            }
                        }),
                    ),
            ),
        )
        // players add <holder> <objective> <delta>
        .child(
            literal("add").child(
                argument("holder", Parser::MinecraftScoreHolder(Default::default()), false)
                    .child(
                        argument("objective", Parser::MinecraftObjective, false).child(
                            argument("score", Parser::BrigadierInteger(Default::default()), false)
                                .executes({
                                    let ops = ops.clone();
                                    move |ctx| {
                                        submit(&ops, ScoreboardOp::AddScore {
                                            holder: text(ctx, "holder")?.to_string(),
                                            objective: text(ctx, "objective")?.to_string(),
                                            delta: integer(ctx, "score")?,
                                        });
                                        Ok(())
                                    }
                                }),
                        ),
                    ),
            ),
        )
        // players reset <holder> [objective]
        .child(
            literal("reset").child(
                argument("holder", Parser::MinecraftScoreHolder(Default::default()), false)
                    .executes({
                        let ops = ops.clone();
                        move |ctx| {
                            submit(&ops, ScoreboardOp::ResetScore {
                                holder: text(ctx, "holder")?.to_string(),
                                objective: None,
                            });
                            Ok(())
                        }
                    })
                    .child(
                        argument("objective", Parser::MinecraftObjective, false).executes({
                            let ops = ops;
                            move |ctx| {
                                submit(&ops, ScoreboardOp::ResetScore {
                                    holder: text(ctx, "holder")?.to_string(),
                                    objective: Some(text(ctx, "objective")?.to_string()),
                                });
                                Ok(())
                            }
                        }),
                    ),
            ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scoreboard;
    use qexed_command::CommandTree;

    /// dispatch 一行指令并 drain 应用，返回全部副作用。
    fn run(tree: &mut CommandTree, ops: &OpQueue, sb: &mut Scoreboard, line: &str) -> Vec<ScoreboardEffect> {
        tree.dispatch(line).expect("dispatch should succeed");
        let queued: Vec<ScoreboardOp> = ops.lock().unwrap().drain(..).collect();
        let mut out = Vec::new();
        for op in queued {
            out.extend(apply_op(sb, op));
        }
        out
    }

    #[test]
    fn add_list_remove_objective_flow() {
        let ops: OpQueue = Arc::new(Mutex::new(Vec::new()));
        let mut sb = Scoreboard::new();
        let mut tree = CommandTree::new().command(scoreboard_node(ops.clone()));

        let fx = run(&mut tree, &ops, &mut sb, "scoreboard objectives add kills dummy Kills");
        assert!(matches!(&fx[0], ScoreboardEffect::Packet(crate::SyncPacket::Objective(_))));
        assert!(matches!(&fx[1], ScoreboardEffect::Reply(m) if m.contains("kills")));
        assert!(sb.has_objective("kills"));

        // 重名拒绝
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard objectives add kills dummy");
        assert!(matches!(&fx[0], ScoreboardEffect::Rejected(m) if m.contains("already exists")));

        // 列表
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard objectives list");
        assert!(matches!(&fx[0], ScoreboardEffect::Reply(m) if m.contains("1 objectives")));

        // 删除
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard objectives remove kills");
        assert!(matches!(&fx[0], ScoreboardEffect::Packet(crate::SyncPacket::Objective(_))));
        assert!(!sb.has_objective("kills"));
    }

    #[test]
    fn score_set_get_add_reset_flow() {
        let ops: OpQueue = Arc::new(Mutex::new(Vec::new()));
        let mut sb = Scoreboard::new();
        let mut tree = CommandTree::new().command(scoreboard_node(ops.clone()));
        run(&mut tree, &ops, &mut sb, "scoreboard objectives add money dummy Money");

        let fx = run(&mut tree, &ops, &mut sb, "scoreboard players set alice money 10");
        assert!(matches!(&fx[0], ScoreboardEffect::Packet(crate::SyncPacket::Score(_))));
        assert_eq!(sb.get_score("money", "alice"), Some(10));

        let fx = run(&mut tree, &ops, &mut sb, "scoreboard players add alice money -3");
        assert_eq!(sb.get_score("money", "alice"), Some(7));
        assert!(matches!(&fx[1], ScoreboardEffect::Reply(m) if m.contains("7")));

        let fx = run(&mut tree, &ops, &mut sb, "scoreboard players get alice money");
        assert!(matches!(&fx[0], ScoreboardEffect::Reply(m) if m.contains("= 7")));

        // players list（全体持有者）
        run(&mut tree, &ops, &mut sb, "scoreboard players set bob money 1");
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard players list");
        assert!(matches!(&fx[0], ScoreboardEffect::Reply(m) if m.contains("2 tracked")));

        // 单持有者列表
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard players list alice");
        assert!(matches!(&fx[0], ScoreboardEffect::Reply(m) if m.contains("alice")));

        // reset 单目标
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard players reset alice money");
        assert!(matches!(&fx[0], ScoreboardEffect::Packet(crate::SyncPacket::Reset(_))));
        assert_eq!(sb.get_score("money", "alice"), None);

        // reset 全部
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard players reset bob");
        assert!(matches!(&fx[0], ScoreboardEffect::Packet(crate::SyncPacket::Reset(_))));
        assert!(sb.holders().is_empty());
    }

    #[test]
    fn setdisplay_and_modify_flow() {
        let ops: OpQueue = Arc::new(Mutex::new(Vec::new()));
        let mut sb = Scoreboard::new();
        let mut tree = CommandTree::new().command(scoreboard_node(ops.clone()));
        run(&mut tree, &ops, &mut sb, "scoreboard objectives add a dummy");

        let fx = run(&mut tree, &ops, &mut sb, "scoreboard objectives setdisplay sidebar a");
        assert!(matches!(
            &fx[0],
            ScoreboardEffect::Packet(crate::SyncPacket::Display(d)) if d.slot.0 == crate::slot::SIDEBAR
        ));
        assert_eq!(sb.display(crate::slot::SIDEBAR), Some("a"));

        // 清空槽位
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard objectives setdisplay sidebar");
        assert!(matches!(
            &fx[0],
            ScoreboardEffect::Packet(crate::SyncPacket::Display(d)) if d.objective_name.is_empty()
        ));

        // 改名
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard objectives modify a displayname Fancy A");
        assert!(matches!(&fx[0], ScoreboardEffect::Packet(crate::SyncPacket::Objective(_))));

        // 未知槽位
        let fx = run(&mut tree, &ops, &mut sb, "scoreboard objectives setdisplay nowhere a");
        assert!(matches!(&fx[0], ScoreboardEffect::Rejected(m) if m.contains("slot")));
    }

    #[test]
    fn unknown_objective_rejected_and_usage_errors() {
        let ops: OpQueue = Arc::new(Mutex::new(Vec::new()));
        let mut sb = Scoreboard::new();
        let mut tree = CommandTree::new().command(scoreboard_node(ops.clone()));

        let fx = run(&mut tree, &ops, &mut sb, "scoreboard players set p ghost 1");
        assert!(matches!(&fx[0], ScoreboardEffect::Rejected(m) if m.contains("ghost")));

        // 缺参数 → Usage
        assert!(matches!(
            tree.dispatch("scoreboard players set"),
            Err(CommandError::InvalidUsage { .. })
        ));
        // 未知子命令 → UnknownCommand（scoreboard 是已知指令，players 后的坏分支回 Usage）
        assert!(tree.dispatch("scoreboard nonsense").is_err());
        // 分数不是整数 → BadArgument
        assert!(matches!(
            tree.dispatch("scoreboard players set p obj abc"),
            Err(CommandError::BadArgument { .. })
        ));
    }
}