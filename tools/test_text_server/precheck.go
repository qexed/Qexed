package main

import (
	"regexp"
	"strings"
)

// ============================================================
// 第一级：关键词 / 正则预筛（微秒级）
// ============================================================

type PreCheckResult struct {
	Suspicious bool
	Severity   string // "low" / "medium" / "high"
	Rule       string // 命中的规则名
}

var (
	reCNNumber = regexp.MustCompile(`[零一二三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟两]`)

	// --- 涉黄：露骨脏词 + 性骚扰（仅拦截最硬的，避免误伤正常社交）---
	reNSFW = regexp.MustCompile(`(?i)(草\s*(你|尼|拟|me|我)|fuck|操\s*(你|尼|拟)|傻逼|煞笔|sb\b|cnm|cao|肏|屌|鸡巴|jb\b|草泥马|妈逼|你妈|日你|婊子|妓女|卖淫|嫖娼|裸聊|约炮|一夜情|性交|口交|肛交|骚货|骚逼|发骚|好骚|骚啊|色狼|色鬼|猥亵|性骚扰|淫秽|荡妇)`)

	// --- RMT：数字 + r/R + 金钱单位 ---
	reRMT = regexp.MustCompile(`\d+\s*[rR]\b|[rR][mM][bB]|\d+\s*(元|块|¥|💰)`)

	// --- 广告：q群 / QQ群 / 群号 + 各种数字形式 ---
	reAdGroup = regexp.MustCompile(`(?i)([qQ扣]\s*群|QQ\s*群|群号|qqun|企鹅群)\s*[：:]*\s*[\w零一二三四五六七八九十壹贰叁肆伍陆柒捌玖拾]+`)

	// --- 广告：微信号 / 链接 ---
	reAdContact = regexp.MustCompile(`(?i)(微信|v信|vx|wx|微)\s*[：:]*\s*\S{3,}|https?://|www\.\S+\.(com|cn|net|cc|gg|xyz|top)`)

	// --- 广告：价格 + 物品（如"20r一组""10块钱"）---
	rePriceTag = regexp.MustCompile(`\d+\s*[rR]\s*[一俩两三]?(组|个|把|套|件|箱|车|堆)|[rR][mM][bB]\s*\d+`)

	// --- 违法：赌博/毒品关键词 ---
	reIllegal = regexp.MustCompile(`(?i)(赌场|赌博|下注|押注|彩票|六合彩|毒品|大麻|冰毒|海洛因|摇头丸|k粉|代考|替考|枪|炸药)`)
)

func preCheck(text string) PreCheckResult {
	if strings.TrimSpace(text) == "" {
		return PreCheckResult{}
	}

	lower := strings.ToLower(text)

	// 涉黄 → high severity
	if reNSFW.MatchString(lower) {
		return PreCheckResult{Suspicious: true, Severity: "high", Rule: "涉黄"}
	}

	// 违法 → high severity
	if reIllegal.MatchString(lower) {
		return PreCheckResult{Suspicious: true, Severity: "high", Rule: "违法"}
	}

	// RMT → medium
	if reRMT.MatchString(text) || rePriceTag.MatchString(text) {
		return PreCheckResult{Suspicious: true, Severity: "medium", Rule: "RMT"}
	}

	// 广告：群号 → medium
	if reAdGroup.MatchString(text) {
		return PreCheckResult{Suspicious: true, Severity: "medium", Rule: "广告"}
	}

	// 广告：微信号/链接 → medium
	if reAdContact.MatchString(text) {
		return PreCheckResult{Suspicious: true, Severity: "medium", Rule: "广告"}
	}

	// 中文数字 ≥ 3 个且消息较短（疑似群号变体如"四六五九四四"）
	cnCount := len(reCNNumber.FindAllString(text, -1))
	if cnCount >= 3 && len([]rune(text)) <= 30 {
		return PreCheckResult{Suspicious: true, Severity: "low", Rule: "广告(数字变体)"}
	}

	return PreCheckResult{}
}
