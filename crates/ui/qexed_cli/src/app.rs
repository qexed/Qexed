use anyhow::Result;
use chrono::Local;
use crossterm::event::{KeyCode};
use std::sync::{Arc, Mutex};

/// CLI应用状态
pub struct CliApp {
    pub logs: Vec<String>,                         // 日志记录
    input: String,                                 // 命令行输入
    pub log_scroll: usize,                         // 日志滚动位置
    pub max_log_lines: usize,                      // 最大日志行数
    pub auto_scroll:bool,// 最大滚动行
    exit: bool,                                    // 退出标志（由上层控制）
    input_cursor_position: usize,                  // 输入光标位置
    input_height: u16,                             // 输入区域高度
    min_input_height: u16,                         // 最小输入区域高度
    max_input_height: u16,                         // 最大输入区域高度
    command_tx: tokio::sync::mpsc::Sender<String>, // 命令发送通道
    pub name:&'static str,
}

pub type CliAppRef = Arc<Mutex<CliApp>>;

impl CliApp {
    /// 创建新的CLI应用
    pub fn new(command_tx: tokio::sync::mpsc::Sender<String>,name:&'static str) -> CliAppRef {
        Arc::new(Mutex::new(Self {
            logs: Vec::new(),
            input: String::new(),
            log_scroll: 0,
            max_log_lines: 1000,
            exit: false,
            input_cursor_position: 0,
            input_height: 5,
            min_input_height: 3,
            max_input_height: 20,
            command_tx,
            auto_scroll: true,
            name:name,
        }))
    }

    /// 添加日志消息
    pub fn add_log(&mut self, message: String) {
        let now = Local::now();
        let timestamp = now.format("%H:%M:%S").to_string();
        let log_entry = format!("[{}] {}", timestamp, message);
        let old_len = self.logs.len();
        let current_scroll = self.log_scroll;
        self.logs.push(log_entry);

        if self.logs.len() > self.max_log_lines {
            self.logs.remove(0);
        }
        // 若为滚动条最底部，则自动滚动
        if old_len == 0 || current_scroll == old_len - 1 {
            self.log_scroll = self.logs.len().saturating_sub(1);
        }
    }

    /// 设置退出标志（由上层调用）
    pub fn set_exit(&mut self, exit: bool) {
        self.exit = exit;
    }

    /// 提交用户输入的命令
    pub async fn submit_command(&mut self) -> Result<()> {
        if self.input.trim().is_empty() {
            return Ok(());
        }

        let command = self.input.trim().to_string();

        // 清空输入
        self.input.clear();
        self.input_cursor_position = 0;
        self.input_height = 5;
        if command.starts_with("::qexed_cli::"){
            log::error!("禁止直接使用内部命令");
            return Ok(());
        }
        // 发送命令到上层
        if let Err(e) = self.command_tx.send(command.clone()).await {
            self.add_log(format!("发送命令失败: {}", e));
            return Ok(());
        }

        Ok(())
    }

    /// 获取日志快照
    pub fn get_logs_snapshot(&self) -> Vec<String> {
        self.logs.clone()
    }

    /// 检查是否需要退出
    pub fn should_exit(&self) -> bool {
        self.exit
    }

    /// 处理键盘事件
    pub async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                self.submit_command().await?;
            }
            KeyCode::Char(c) => {
                let byte_pos = self.char_index_to_byte_index(self.input_cursor_position);
                self.input.insert(byte_pos, c);
                self.input_cursor_position += 1;
            }
            KeyCode::Backspace => {
                if self.input_cursor_position > 0 && !self.input.is_empty() {
                    let byte_pos = self.char_index_to_byte_index(self.input_cursor_position - 1);

                    let mut char_len = 1;
                    if let Some(ch) = self.input[byte_pos..].chars().next() {
                        char_len = ch.len_utf8();
                    }

                    self.input.drain(byte_pos..byte_pos + char_len);
                    self.input_cursor_position -= 1;
                }
            }
            KeyCode::Delete => {
                let char_count = self.input.chars().count();
                if self.input_cursor_position < char_count {
                    let byte_pos = self.char_index_to_byte_index(self.input_cursor_position);

                    let mut char_len = 1;
                    if let Some(ch) = self.input[byte_pos..].chars().next() {
                        char_len = ch.len_utf8();
                    }

                    self.input.drain(byte_pos..byte_pos + char_len);
                }
            }
            KeyCode::Left => {
                if self.input_cursor_position > 0 {
                    self.input_cursor_position -= 1;
                }
            }
            KeyCode::Right => {
                let char_count = self.input.chars().count();
                if self.input_cursor_position < char_count {
                    self.input_cursor_position += 1;
                }
            }
            KeyCode::Home => {
                self.input_cursor_position = 0;
            }
            KeyCode::End => {
                self.input_cursor_position = self.input.chars().count();
            }
            KeyCode::Up => {
                self.auto_scroll = false;
                if self.log_scroll > 0 {
                    self.log_scroll -= 1;
                }
            }
            KeyCode::Down => {
                let logs_guard = &self.logs;

                if self.log_scroll < logs_guard.len().saturating_sub(1) {
                    self.log_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.log_scroll = self.log_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.log_scroll = (self.log_scroll + 10).min(self.logs.len().saturating_sub(1));
            }
            KeyCode::Tab => {
                let byte_pos = self.char_index_to_byte_index(self.input_cursor_position);
                self.input.insert_str(byte_pos, "    ");
                self.input_cursor_position += 4;
            }
            KeyCode::Esc => {
                // ESC键只发送退出命令，但不直接退出
                let _ = self.command_tx.send("exit".to_string()).await;
            }
            _ => {}
        }

        Ok(())
    }

    /// 处理鼠标滚轮事件
    pub fn handle_mouse_scroll(&mut self, lines: i32) {
        let log_len = self.logs.len();

        if lines > 0 {
            self.auto_scroll = false;
            // ScrollUp: 向上滚动，查看旧日志
            if self.log_scroll > 0 {
                self.log_scroll = self.log_scroll.saturating_sub(lines as usize);
            }
        } else if lines < 0 {
            // ScrollDown: 向下滚动，查看新日志
            self.log_scroll = (self.log_scroll + (-lines) as usize).min(log_len.saturating_sub(1));
        }
    }

    /// 更新输入区域高度
    pub fn update_input_height(&mut self, area_height: u16) {
        self.input_height = self.calculate_input_height(area_height);
    }

    /// 获取输入区域高度
    pub fn input_height(&self) -> u16 {
        self.input_height
    }

    /// 获取输入文本
    pub fn input_text(&self) -> String {
        format!("> {}", self.input)
    }

    /// 获取输入光标位置
    pub fn input_cursor_position(&self) -> usize {
        self.input_cursor_position
    }

    /// 获取当前日志滚动位置
    pub fn log_scroll(&self) -> usize {
        self.log_scroll
    }

    pub fn update_log_scroll(&mut self, log_scroll: usize) {
        self.log_scroll = log_scroll
    }

    fn calculate_input_height(&self, area_height: u16) -> u16 {
        if self.input.is_empty() {
            let default_height = (area_height / 10).max(3).min(6);
            return default_height;
        }

        let lines: Vec<&str> = self.input.lines().collect();
        let line_count = lines.len() as u16;

        let calculated_height = (line_count + 2).max(3).min(self.max_input_height);
        let max_reasonable_height = (area_height * 4) / 10;
        calculated_height
            .min(max_reasonable_height)
            .max(self.min_input_height)
    }

    fn char_index_to_byte_index(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }

        let mut chars_counted = 0;

        for (idx, _) in self.input.char_indices() {
            if chars_counted == char_index {
                return idx;
            }
            chars_counted += 1;
        }

        if chars_counted == char_index {
            return self.input.len();
        }

        self.input.len()
    }
}
