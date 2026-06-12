package main

import (
	"encoding/json"
	"log"
	"os"
	"sync"
	"time"
)

// ============================================================
// 玩家信誉系统
// ============================================================

type PlayerRep struct {
	Reputation    int       `json:"reputation"`
	LastViolation time.Time `json:"last_violation"`
	LastSeen      time.Time `json:"last_seen"`
	Violations    int       `json:"violations"`    // 累计违规次数
}

type ReputationStore struct {
	mu      sync.RWMutex
	players map[string]*PlayerRep // playerName → rep
	path    string
}

var repStore = &ReputationStore{
	players: make(map[string]*PlayerRep),
	path:    "reputation.json",
}

func (rs *ReputationStore) load() {
	data, err := os.ReadFile(rs.path)
	if err != nil {
		if os.IsNotExist(err) {
			return
		}
		log.Printf("加载信誉数据失败: %v", err)
		return
	}
	if err := json.Unmarshal(data, &rs.players); err != nil {
		log.Printf("解析信誉数据失败: %v", err)
	}
	log.Printf("已加载 %d 条玩家信誉记录", len(rs.players))
}

func (rs *ReputationStore) save() {
	rs.mu.RLock()
	data, err := json.MarshalIndent(rs.players, "", "  ")
	rs.mu.RUnlock()
	if err != nil {
		log.Printf("序列化信誉数据失败: %v", err)
		return
	}
	if err := os.WriteFile(rs.path, data, 0644); err != nil {
		log.Printf("写入信誉文件失败: %v", err)
	}
}

// get 获取玩家信誉分，不存在则初始化
func (rs *ReputationStore) get(player string) int {
	rs.mu.Lock()
	defer rs.mu.Unlock()

	rep, ok := rs.players[player]
	if !ok {
		rs.players[player] = &PlayerRep{
			Reputation: reputationInit,
			LastSeen:   time.Now(),
		}
		return reputationInit
	}

	// 时间恢复：上次违规后经过 N 小时自动恢复
	if !rep.LastViolation.IsZero() && time.Since(rep.LastViolation) > recoverInterval {
		intervals := int(time.Since(rep.LastViolation) / recoverInterval)
		recovered := intervals * recoverAmount
		if recovered > 0 {
			rep.Reputation += recovered
			if rep.Reputation > reputationMax {
				rep.Reputation = reputationMax
			}
			rep.LastViolation = time.Now() // 重置计时
		}
	}

	rep.LastSeen = time.Now()
	return rep.Reputation
}

// penalize 违规扣分
func (rs *ReputationStore) penalize(player string, severity string) int {
	rs.mu.Lock()
	defer rs.mu.Unlock()

	rep, ok := rs.players[player]
	if !ok {
		rep = &PlayerRep{Reputation: reputationInit}
		rs.players[player] = rep
	}

	var penalty int
	switch severity {
	case "high":
		penalty = penaltySevere
	case "medium":
		penalty = penaltyModerate
	default:
		penalty = penaltyMinor
	}

	rep.Reputation -= penalty
	if rep.Reputation < reputationMin {
		rep.Reputation = reputationMin
	}
	rep.LastViolation = time.Now()
	rep.Violations++

	log.Printf("玩家 %s 信誉 -%d → %d（累计违规 %d 次）", player, penalty, rep.Reputation, rep.Violations)

	go rs.save() // 异步落盘
	return rep.Reputation
}

// getPunishment 根据当前信誉分返回惩罚级别
func (rs *ReputationStore) getPunishment(player string) punishLevel {
	rep := rs.get(player)
	for _, lv := range punishLevels {
		if rep >= lv.MinRep {
			return lv
		}
	}
	return punishLevels[len(punishLevels)-1]
}
