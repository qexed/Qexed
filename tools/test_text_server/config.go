package main

import "time"

// ============================================================
// 审核管道配置
// ============================================================

const (
	// 服务端口
	listenAddr = ":9800"

	// Ollama 本地模型（第一级 LLM，快速粗筛）
	ollamaURL   = "http://localhost:11434/api/chat"
	ollamaModel = "qwen2.5:0.5b-instruct-q4_K_M"
	ollamaTimeout = 10 * time.Second

	// DeepSeek API（第二级 LLM，终审）
	deepseekURL   = "https://api.deepseek.com/v1/chat/completions"
	deepseekModel = "deepseek-v4-pro"
	deepseekTimeout = 15 * time.Second
)

// ============================================================
// 信誉系统配置
// ============================================================

const (
	reputationInit    = 50    // 初始信誉分（新号不信任，必须过审）
	reputationMin     = 0     // 最低信誉分
	reputationMax     = 100   // 最高信誉分

	// 违规扣分
	penaltyMinor  = 10   // 轻微违规（预筛命中但 LLM 初审放行的）
	penaltyModerate = 25 // 中等违规（LLM 确认违规）
	penaltySevere = 40   // 严重违规（涉黄/违法）

	// 时间恢复：每 N 小时恢复 1 分（未违规期间）
	recoverInterval = 1 * time.Hour
	recoverAmount   = 2
)

// ============================================================
// 惩罚系统配置
// ============================================================

// 信誉分区间 → 惩罚级别
type punishLevel struct {
	MinRep  int
	Action  string // "none" | "warn" | "mute" | "kick" | "ban"
	MuteDur time.Duration // 禁言时长，仅 mute 生效
}

var punishLevels = []punishLevel{
	{MinRep: 80, Action: "none"},
	{MinRep: 60, Action: "warn"},
	{MinRep: 40, Action: "mute", MuteDur: 5 * time.Minute},
	{MinRep: 20, Action: "mute", MuteDur: 30 * time.Minute},
	{MinRep: 0,  Action: "ban"},
}

// ============================================================
// RCON 配置（用于在 qexed 上执行惩罚命令）
// ============================================================

const (
	rconEnable   = false // 设为 true 启用自动执法
	rconHost     = "127.0.0.1:25575"
	rconPassword = ""
)

// ============================================================
// 审核规则预筛配置
// ============================================================

// 预筛仅拦截 high severity（涉黄/违法硬词），其余全部走 AI 判决
