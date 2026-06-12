package main

import (
	"log"
	"net/http"
	"strings"

	"github.com/gin-gonic/gin"
)

// ============================================================
// API 定义
// ============================================================

// 请求（兼容 qexed content_filter 的原始格式 + 扩展字段）
type CheckRequest struct {
	Text   string `json:"text" form:"text"`
	Player string `json:"player" form:"player"` // 玩家名，用于信誉追踪
}

// 响应（兼容原始三字段 + 扩展）
type CheckResponse struct {
	Block        bool     `json:"block"`
	FilteredText string   `json:"filtered_text,omitempty"`
	Reason       string   `json:"reason,omitempty"`
	RiskScore    int      `json:"risk_score,omitempty"`
	Rules        []string `json:"rules,omitempty"`
	Punishment   string   `json:"punishment,omitempty"`
	Reputation   int      `json:"reputation,omitempty"`
	Level        string   `json:"level,omitempty"`
}

func setupRoutes(r *gin.Engine) {
	r.POST("/check", handleCheck)
	r.GET("/health", handleHealth)
	r.GET("/reputation/:player", handleGetReputation)
}

func handleCheck(c *gin.Context) {
	var req CheckRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		// 兼容纯 text 字符串（某些客户端只发 {"text": "..."}）
		c.JSON(http.StatusBadRequest, CheckResponse{
			Block:  true,
			Reason: "bad request: " + err.Error(),
		})
		return
	}

	text := strings.TrimSpace(req.Text)
	if text == "" {
		c.JSON(http.StatusOK, CheckResponse{Block: false})
		return
	}

	player := strings.TrimSpace(req.Player)
	if player == "" {
		player = "anonymous"
	}

	result := reviewMessage(text, player)

	log.Printf("[审核] player=%s rep=%d level=%s block=%v risk=%d rules=%v punishment=%s text=%s",
		player, result.Reputation, result.Level, result.Block,
		result.RiskScore, result.Rules, result.Punishment, truncate(text, 80))

	c.JSON(http.StatusOK, CheckResponse{
		Block:        result.Block,
		FilteredText: result.FilteredText,
		Reason:       result.Reason,
		RiskScore:    result.RiskScore,
		Rules:        result.Rules,
		Punishment:   result.Punishment,
		Reputation:   result.Reputation,
		Level:        result.Level,
	})
}

func handleHealth(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "ok"})
}

func handleGetReputation(c *gin.Context) {
	player := c.Param("player")
	rep := repStore.get(player)
	c.JSON(http.StatusOK, gin.H{
		"player":     player,
		"reputation": rep,
	})
}
