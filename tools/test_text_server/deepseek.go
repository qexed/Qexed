package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strings"
)

// ============================================================
// 第三级：DeepSeek API 终审
// ============================================================

type deepseekMsg struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

type deepseekReq struct {
	Model    string        `json:"model"`
	Messages []deepseekMsg `json:"messages"`
	Stream   bool          `json:"stream"`
	MaxTokens int          `json:"max_tokens"`
	Temperature float64    `json:"temperature"`
}

type deepseekResp struct {
	Choices []struct {
		Message      deepseekMsg `json:"message"`
		FinishReason string      `json:"finish_reason"`
	} `json:"choices"`
}

const deepseekSystemPrompt = `You are a Minecraft chat moderator doing final review. The message was flagged as suspicious by a fast pre-screener.

Rules:
1. NSFW - explicit sexual content only. Normal dating/flirting ("脱单""找对象""cpdd") is NOT NSFW.
2. ILLEGAL - violence, gambling, drugs, fraud.
3. AD - advertising QQ groups, WeChat, links, phone numbers. Normal in-game barter is fine.
4. RMT - real money trading. "r"/"R" means RMB yuan. "元""块""¥""rmb""💰" are money units. Any item pricing with real currency = RMT.

Output ONLY valid JSON, no markdown:
{"block":true/false,"risk":0-100,"rule":"nsfw/illegal/ad/rmt/clean","reason":"brief Chinese reason if blocked, else empty"}`

var (
	deepseekAPIKey = getDeepseekKey()
	deepseekClient = &http.Client{Timeout: deepseekTimeout}
)

func getDeepseekKey() string {
	if k := os.Getenv("DEEPSEEK_API_KEY"); k != "" {
		return k
	}
	return "sk-2e88af1af9a34f49810e9926888b9195"
}

// DeepSeekVerdict 终审判决
type DeepSeekVerdict struct {
	Block  bool   `json:"block"`
	Risk   int    `json:"risk"`
	Rule   string `json:"rule"`
	Reason string `json:"reason"`
}

func checkDeepSeek(text string) (*DeepSeekVerdict, error) {
	if deepseekAPIKey == "" {
		return nil, fmt.Errorf("DEEPSEEK_API_KEY 未设置")
	}

	req := deepseekReq{
		Model: deepseekModel,
		Messages: []deepseekMsg{
			{Role: "system", Content: deepseekSystemPrompt},
			{Role: "user", Content: text},
		},
		Stream:      false,
		MaxTokens:   300,
		Temperature: 0.0,
	}

	body, err := json.Marshal(req)
	if err != nil {
		return nil, fmt.Errorf("marshal: %w", err)
	}

	httpReq, err := http.NewRequest("POST", deepseekURL, bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("create req: %w", err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+deepseekAPIKey)
	httpReq.Header.Set("Content-Type", "application/json")

	resp, err := deepseekClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("call: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		raw, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("status %d: %s", resp.StatusCode, string(raw))
	}

	var chatResp deepseekResp
	if err := json.NewDecoder(resp.Body).Decode(&chatResp); err != nil {
		return nil, fmt.Errorf("decode: %w", err)
	}
	if len(chatResp.Choices) == 0 {
		return nil, fmt.Errorf("no choices")
	}

	choice := chatResp.Choices[0]
	content := strings.TrimSpace(choice.Message.Content)

	if content == "" {
		log.Printf("DeepSeek 空回复，finish_reason=%s，按拦截处理", choice.FinishReason)
		return &DeepSeekVerdict{Block: true, Risk: 80, Rule: "content_filter", Reason: "消息被内容安全系统拦截"}, nil
	}

	var v DeepSeekVerdict
	if err := json.Unmarshal([]byte(content), &v); err != nil {
		return nil, fmt.Errorf("parse JSON: %w (raw: %s)", err, truncate(content, 200))
	}
	return &v, nil
}
