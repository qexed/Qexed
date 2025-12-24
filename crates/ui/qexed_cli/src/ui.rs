use crate::app::CliAppRef;

use super::app::CliApp;
use anyhow::Result;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};
use std::time::Duration;
use regex::Regex;
/// 渲染CLI界面
pub fn render(frame: &mut Frame, app_ref: &CliAppRef) {
    let mut app = app_ref.lock().unwrap();
    let area = frame.area();

    if area.height < 8 {
        frame.render_widget(
            Paragraph::new("终端窗口太小，请调整大小")
                .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    }

    app.update_input_height(area.height);
    let input_height = app.input_height();
    let log_height = area.height.saturating_sub(input_height);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(log_height),
            Constraint::Length(input_height),
        ])
        .split(area);

    render_log_area(frame, &mut app, chunks[0]);
    render_input_area(frame, &app, chunks[1]);
}

/// 渲染日志区域
fn render_log_area(frame: &mut Frame, app: &mut CliApp, area: Rect) {
    let logs_snapshot = app.get_logs_snapshot();

    // 计算内部可用高度（减去边框）
    let inner_height = area.height.saturating_sub(2) as usize; // 上下边框各占1行

    // 如果日志为空，显示空状态
    if logs_snapshot.is_empty() {
        let empty_message = "暂无日志";
        let empty_widget = Paragraph::new(empty_message)
            .block(
                Block::default()
                    .title(app.name)
                    .borders(Borders::ALL)
                    .style(Style::default()),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(empty_widget, area);
        return;
    }

    // 自动滚动到底部
    let mut start_index = app.log_scroll();
    let mut total_logs = logs_snapshot.len();
    if total_logs > inner_height {
        if start_index > (total_logs - inner_height) {
            if inner_height != 0 {
                start_index = total_logs - inner_height;
                app.update_log_scroll(start_index);
                if total_logs >= inner_height {
                    total_logs -= inner_height;
                }
            }
        }
    }

    let mut start_index = if start_index < total_logs {
        if logs_snapshot.len() > inner_height {
            start_index
        } else {
            0
        }
    } else if total_logs > 0 {
        total_logs
    } else {
        0
    };
    if app.auto_scroll == true {
        if total_logs > inner_height {
            start_index = total_logs - inner_height
        }
    }
    let log_lines: Vec<Line> = logs_snapshot
        .iter()
        .skip(start_index)
        .take(inner_height)
        .enumerate()
        // 然后在函数内部
        .map(|(i, log)| {
            let line_number = start_index + i + 1;

            // 创建正则表达式匹配日志级别
            let re = Regex::new(r"(\[INFO\]|\[WARN\]|\[ERROR\]|\[DEBUG\])").unwrap();

            // 初始化行向量
            let mut line_spans = vec![Span::styled(
                format!("{:4} ", line_number),
                Style::default().fg(Color::DarkGray),
            )];

            let mut last_end = 0;

            // 查找所有匹配的日志级别
            for mat in re.find_iter(log) {
                // 添加匹配前的文本（默认颜色）
                if mat.start() > last_end {
                    line_spans.push(Span::styled(&log[last_end..mat.start()], Style::default()));
                }

                // 为匹配到的日志级别添加颜色
                let match_text = mat.as_str();
                let level_color = match match_text {
                    "[INFO]" => Color::Green,
                    "[WARN]" => Color::Yellow,
                    "[ERROR]" => Color::Red,
                    "[DEBUG]" => Color::Blue,
                    _ => Color::White, // 默认颜色
                };

                line_spans.push(Span::styled(match_text, Style::default().fg(level_color)));

                last_end = mat.end();
            }

            // 添加剩余的文本
            if last_end < log.len() {
                line_spans.push(Span::styled(&log[last_end..], Style::default()));
            }

            Line::from(line_spans)
        })
        .collect();

    let logs = Text::from(log_lines);

    let log_widget = Paragraph::new(logs)
        .block(
            Block::default()
                .title(app.name)
                .borders(Borders::ALL)
                .style(Style::default()),
        )
        .wrap(Wrap { trim: true })
        .scroll((0, 0));

    frame.render_widget(log_widget, area);

    // 如果需要显示滚动条
    if (total_logs > inner_height) && (total_logs > 0) {
        if total_logs - inner_height == start_index {
            if app.auto_scroll == false {
                app.auto_scroll = true;
            }
        }

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(total_logs - inner_height)
            .position(start_index)
            .content_length(total_logs - inner_height);

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut scrollbar_state,
        );
    }
}

/// 渲染输入区域
fn render_input_area(frame: &mut Frame, app: &CliApp, area: Rect) {
    let input_text = app.input_text();
    let input_lines: Vec<Line> = input_text.lines().map(|line| Line::from(line)).collect();

    let input_widget = Paragraph::new(input_lines)
        .block(
            Block::default()
                .title(" 命令输入 ")
                .borders(Borders::ALL)
                .style(Style::default()),
        )
        .style(Style::default().fg(Color::Yellow))
        .wrap(Wrap { trim: true });

    frame.render_widget(input_widget, area);

    // 计算并设置光标位置
    let cursor_pos = calculate_cursor_position(app, area);
    if let Some((x, y)) = cursor_pos {
        frame.set_cursor_position((x, y));
    }
}

/// 计算光标位置
fn calculate_cursor_position(app: &CliApp, area: Rect) -> Option<(u16, u16)> {
    let prompt_width = 2; // "> "的长度
    let input_text = app.input_text();
    let cursor_char_index = app.input_cursor_position() + prompt_width;

    let lines: Vec<&str> = input_text.lines().collect();
    let current_line = 0;
    let mut current_column = 0;
    let mut chars_processed = 0;
    if lines.len() > 0 {
        for (_line_idx, line) in lines.iter().enumerate() {
            for (_col_idx, ch) in line.char_indices() {
                if chars_processed >= cursor_char_index {
                    return Some((
                        area.x + 1 + current_column,
                        area.y + 1 + current_line as u16,
                    ));
                }

                let char_width = if is_full_width_char(ch) { 2 } else { 1 };
                current_column += char_width;
                // if col_idx + 1 >= line.len() {
                //     // 如果是行末，换行
                //     current_column = 0;
                // } else {
                //     current_column += char_width;
                // }

                chars_processed += 1;
            }
        }
        // 光标在末尾
        if chars_processed >= cursor_char_index {
            return Some((
                area.x + 1 + current_column,
                area.y + 1 + current_line as u16,
            ));
        }
    } else {
        // current_column = 0
    }

    None
}

/// 判断字符是否为全角字符
fn is_full_width_char(c: char) -> bool {
    match c {
        c if ('\u{4e00}'..='\u{9fff}').contains(&c) => true,
        c if ('\u{3000}'..='\u{303f}').contains(&c) => true,
        c if ('\u{3040}'..='\u{309f}').contains(&c) => true,
        c if ('\u{30a0}'..='\u{30ff}').contains(&c) => true,
        c if ('\u{ac00}'..='\u{d7af}').contains(&c) => true,
        c if ('\u{ff00}'..='\u{ffef}').contains(&c) => true,
        '\u{3001}' | '\u{3002}' | '\u{ff0c}' | '\u{ff0e}' | '\u{ff1a}' | '\u{ff1b}'
        | '\u{ff01}' | '\u{ff1f}' | '\u{ff08}' | '\u{ff09}' | '\u{300c}' | '\u{300d}'
        | '\u{300e}' | '\u{300f}' | '\u{2018}' | '\u{2019}' | '\u{201c}' | '\u{201d}' => true,
        _ => false,
    }
}

/// 运行CLI主循环
pub async fn run_cli_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app_ref: &crate::app::CliAppRef,
) -> Result<()> {
    loop {
        {
            let app = app_ref.lock().unwrap();
            if app.should_exit() {
                break;
            }
        }

        terminal.draw(|frame| render(frame, app_ref))?;

        if crossterm::event::poll(Duration::from_millis(16))? {
            match crossterm::event::read()? {
                crossterm::event::Event::Key(key)
                    if key.kind == crossterm::event::KeyEventKind::Press =>
                {
                    let mut app = app_ref.lock().unwrap();
                    app.handle_key_event(key).await?;
                }
                crossterm::event::Event::Mouse(mouse_event) => {
                    let mut app = app_ref.lock().unwrap();
                    match mouse_event.kind {
                        // 修正滚轮方向
                        crossterm::event::MouseEventKind::ScrollUp => app.handle_mouse_scroll(3), // 向上滚
                        crossterm::event::MouseEventKind::ScrollDown => app.handle_mouse_scroll(-3), // 向下滚
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    Ok(())
}
