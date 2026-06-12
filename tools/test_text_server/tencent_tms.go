package main

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strings"
	"time"
)

// ============================================================
// 腾讯云 TMS 文本审核
// ============================================================

const (
	tmsEndpoint  = "tms.tencentcloudapi.com"
	tmsAction    = "TextModeration"
	tmsVersion   = "2020-12-29"
	tmsRegion    = "ap-guangzhou"
	tmsService   = "tms"
	tmsTimeout   = 5 * time.Second
)

var (
	tmsSecretId  = getEnvOrDefault("TENCENT_SECRET_ID", "")
	tmsSecretKey = getEnvOrDefault("TENCENT_SECRET_KEY", "")
	tmsClient    = &http.Client{Timeout: tmsTimeout}
)

func getEnvOrDefault(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

// ── 请求 ──

type TMSRequest struct {
	Content string `json:"Content"` // Base64
}

// ── 响应 ──

type TMSResponse struct {
	Response struct {
		Suggestion  string          `json:"Suggestion"` // Pass / Block / Review
		Label       string          `json:"Label"`      // Normal / Porn / Ads / Abuse / Illegal / ...
		Keywords    []string        `json:"Keywords"`
		Score       int             `json:"Score"`
		DetailResults []TMSDetail   `json:"DetailResults"`
		RequestId   string          `json:"RequestId"`
		Error       *struct {
			Code    string `json:"Code"`
			Message string `json:"Message"`
		} `json:"Error,omitempty"`
	} `json:"Response"`
}

type TMSDetail struct {
	Label    string   `json:"Label"`
	Keywords []string `json:"Keywords"`
	Score    int      `json:"Score"`
}

// ── TC3-HMAC-SHA256 签名 ──

func signTC3(secretId, secretKey, host, service, action, version, region, payload string, timestamp int64) string {
	// 1. 规范请求串
	httpMethod := "POST"
	canonicalURI := "/"
	canonicalQuery := ""
	canonicalHeaders := fmt.Sprintf("content-type:application/json\nhost:%s\nx-tc-action:%s\n", host, strings.ToLower(action))
	signedHeaders := "content-type;host;x-tc-action"
	hashedPayload := sha256Hex(payload)
	canonicalRequest := fmt.Sprintf("%s\n%s\n%s\n%s\n%s\n%s",
		httpMethod, canonicalURI, canonicalQuery, canonicalHeaders, signedHeaders, hashedPayload)

	// 2. 待签名字符串
	date := time.Unix(timestamp, 0).UTC().Format("2006-01-02")
	credentialScope := fmt.Sprintf("%s/%s/tc3_request", date, service)
	hashedCanonical := sha256Hex(canonicalRequest)
	stringToSign := fmt.Sprintf("TC3-HMAC-SHA256\n%d\n%s\n%s", timestamp, credentialScope, hashedCanonical)

	// 3. 计算签名
	secretDate := hmacSha256([]byte("TC3"+secretKey), []byte(date))
	secretService := hmacSha256(secretDate, []byte(service))
	secretSigning := hmacSha256(secretService, []byte("tc3_request"))
	signature := hex.EncodeToString(hmacSha256(secretSigning, []byte(stringToSign)))

	// 4. Authorization
	authorization := fmt.Sprintf("TC3-HMAC-SHA256 Credential=%s/%s, SignedHeaders=%s, Signature=%s",
		secretId, credentialScope, signedHeaders, signature)

	return authorization
}

func sha256Hex(s string) string {
	h := sha256.Sum256([]byte(s))
	return hex.EncodeToString(h[:])
}

func hmacSha256(key, data []byte) []byte {
	mac := hmac.New(sha256.New, key)
	mac.Write(data)
	return mac.Sum(nil)
}

// ── 调用 TMS ──

func checkTMS(text string) (*ReviewResult, error) {
	if tmsSecretId == "" || tmsSecretKey == "" {
		return nil, fmt.Errorf("未设置 TENCENT_SECRET_ID / TENCENT_SECRET_KEY 环境变量")
	}

	payload := fmt.Sprintf(`{"Content":"%s"}`, base64.StdEncoding.EncodeToString([]byte(text)))
	timestamp := time.Now().Unix()

	authorization := signTC3(tmsSecretId, tmsSecretKey, tmsEndpoint, tmsService,
		tmsAction, tmsVersion, tmsRegion, payload, timestamp)

	req, err := http.NewRequest("POST", "https://"+tmsEndpoint, strings.NewReader(payload))
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	req.Header.Set("Authorization", authorization)
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Host", tmsEndpoint)
	req.Header.Set("X-TC-Action", tmsAction)
	req.Header.Set("X-TC-Timestamp", fmt.Sprintf("%d", timestamp))
	req.Header.Set("X-TC-Version", tmsVersion)
	req.Header.Set("X-TC-Region", tmsRegion)

	resp, err := tmsClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("call TMS: %w", err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)

	var tmsResp TMSResponse
	if err := json.Unmarshal(body, &tmsResp); err != nil {
		return nil, fmt.Errorf("decode TMS response: %w (body: %s)", err, string(body))
	}

	if tmsResp.Response.Error != nil {
		return nil, fmt.Errorf("TMS error [%s]: %s", tmsResp.Response.Error.Code, tmsResp.Response.Error.Message)
	}

	// 映射腾讯云结果到我们的 ReviewResult
	sug := tmsResp.Response.Suggestion
	blocked := sug == "Block"
	risk := tmsResp.Response.Score

	var rules []string
	if tmsResp.Response.Label != "" && tmsResp.Response.Label != "Normal" {
		rules = append(rules, mapTMSLabel(tmsResp.Response.Label))
	}
	for _, d := range tmsResp.Response.DetailResults {
		if d.Label != "" && d.Label != "Normal" {
			rules = append(rules, mapTMSLabel(d.Label))
		}
	}

	reason := ""
	if blocked {
		reason = fmt.Sprintf("内容违规（%s）", strings.Join(rules, "、"))
		if len(tmsResp.Response.Keywords) > 0 {
			reason += " 命中词: " + strings.Join(tmsResp.Response.Keywords, ", ")
		}
	}

	log.Printf("[TMS] suggestion=%s label=%s score=%d keywords=%v",
		sug, tmsResp.Response.Label, risk, tmsResp.Response.Keywords)

	return &ReviewResult{
		Block:      blocked,
		Reason:     reason,
		RiskScore:  risk,
		Rules:      rules,
		Level:      "tms",
	}, nil
}

func mapTMSLabel(label string) string {
	switch label {
	case "Porn":
		return "涉黄"
	case "Ads":
		return "广告"
	case "Abuse":
		return "辱骂"
	case "Illegal":
		return "违法"
	case "Polity":
		return "涉政"
	case "Terror":
		return "暴恐"
	case "Spam":
		return "骚扰"
	default:
		return label
	}
}
