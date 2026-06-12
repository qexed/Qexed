package main

import (
	"log"
)

// ============================================================
// 审核管道：全部消息走腾讯云 TMS（~100ms）
// ============================================================

type ReviewResult struct {
	Block        bool     `json:"block"`
	FilteredText string   `json:"filtered_text"`
	Reason       string   `json:"reason"`
	RiskScore    int      `json:"risk_score"`
	Rules        []string `json:"rules"`
	Punishment   string   `json:"punishment"`
	Reputation   int      `json:"reputation"`
	Level        string   `json:"level"` // "tms" | "precheck"
}

func reviewMessage(text, player string) ReviewResult {
	rep := repStore.get(player)

	// ── 预筛：仅拦截 hard NSFW / 违法（微秒级）──
	pre := preCheck(text)
	if pre.Suspicious && pre.Severity == "high" {
		newRep := repStore.penalize(player, "high")
		punish := repStore.getPunishment(player)
		go rconPunish(player, punish)
		return ReviewResult{
			Block: true, FilteredText: "",
			Reason: "消息包含违规内容（" + pre.Rule + "）",
			RiskScore: 100, Rules: []string{pre.Rule},
			Punishment: punish.Action, Reputation: newRep, Level: "precheck",
		}
	}

	// ── 腾讯云 TMS 审核（~100ms）──
	tmsResult, err := checkTMS(text)
	if err != nil {
		log.Printf("[TMS 降级] 调用失败: %v，兜底放行", err)
		return ReviewResult{
			Block: false, FilteredText: text,
			RiskScore: 0, Rules: nil,
			Punishment: "none", Reputation: rep, Level: "precheck",
		}
	}

	if !tmsResult.Block {
		log.Printf("[TMS] player=%s PASS score=%d text=%s", player, tmsResult.RiskScore, truncate(text, 60))
		return ReviewResult{
			Block: false, FilteredText: text,
			RiskScore: tmsResult.RiskScore, Rules: nil,
			Punishment: "none", Reputation: rep, Level: "tms",
		}
	}

	// TMS 判违规 → 扣分 + 惩罚
	newRep := repStore.penalize(player, "medium")
	punish := repStore.getPunishment(player)
	go rconPunish(player, punish)

	log.Printf("[TMS] player=%s BLOCK score=%d rules=%v reason=%s rep=%d→%d text=%s",
		player, tmsResult.RiskScore, tmsResult.Rules, tmsResult.Reason, rep, newRep, truncate(text, 60))

	return ReviewResult{
		Block: true, FilteredText: "",
		Reason: tmsResult.Reason, RiskScore: tmsResult.RiskScore,
		Rules: tmsResult.Rules, Punishment: punish.Action,
		Reputation: newRep, Level: "tms",
	}
}
