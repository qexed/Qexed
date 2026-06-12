package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
)

// ============================================================
// 第二级：本地 Ollama 快速 LLM 审核
// ============================================================

type ollamaChatReq struct {
	Model    string           `json:"model"`
	Messages []ollamaMessage  `json:"messages"`
	Stream   bool             `json:"stream"`
	Options  map[string]any   `json:"options"`
}

type ollamaMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

type ollamaChatResp struct {
	Message ollamaMessage `json:"message"`
	Done    bool          `json:"done"`
}

const ollamaSystemPrompt = `You are a Minecraft chat moderator. Classify the message:

Rules: no NSFW, no illegal content, no advertising (QQ groups, WeChat, links), no real-money trading (RMT). Normal trading ("diamond for emerald") is fine.

Output ONLY this JSON, nothing else:
{"block":true/false,"risk":0-100,"rule":"nsfw/illegal/ad/rmt/clean","reason":"brief Chinese reason if blocked, else empty"}`

var ollamaClient = &http.Client{Timeout: ollamaTimeout}

// OllamaVerdict Ollama 初审判决
type OllamaVerdict struct {
	Block bool   `json:"block"`
	Risk  int    `json:"risk"`
	Rule  string `json:"rule"`
	Reason string `json:"reason"`
}

func checkOllama(text string) (*OllamaVerdict, error) {
	req := ollamaChatReq{
		Model: ollamaModel,
		Messages: []ollamaMessage{
			{Role: "system", Content: ollamaSystemPrompt},
			{Role: "user", Content: text},
		},
		Stream: false,
		Options: map[string]any{
			"num_predict": 100,
			"temperature": 0.0,
			"num_ctx":     512,
		},
	}

	body, err := json.Marshal(req)
	if err != nil {
		return nil, fmt.Errorf("marshal: %w", err)
	}

	resp, err := ollamaClient.Post(ollamaURL, "application/json", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("call ollama: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		raw, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("ollama %d: %s", resp.StatusCode, string(raw))
	}

	var chatResp ollamaChatResp
	if err := json.NewDecoder(resp.Body).Decode(&chatResp); err != nil {
		return nil, fmt.Errorf("decode: %w", err)
	}

	content := strings.TrimSpace(chatResp.Message.Content)
	var v OllamaVerdict
	if err := json.Unmarshal([]byte(content), &v); err != nil {
		return nil, fmt.Errorf("parse JSON: %w (raw: %s)", err, truncate(content, 200))
	}

	return &v, nil
}

func truncate(s string, n int) string {
	runes := []rune(s)
	if len(runes) <= n {
		return s
	}
	return string(runes[:n]) + "..."
}
