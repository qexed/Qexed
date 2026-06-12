package main

import (
	"encoding/binary"
	"fmt"
	"log"
	"net"
	"time"
)

// ============================================================
// Minecraft RCON 客户端（用于在 qexed 上执行惩罚命令）
// ============================================================

const (
	rconLogin       = 3
	rconCommand     = 2
	rconRespCommand = 0
)

func rconExecute(cmd string) error {
	if !rconEnable {
		return fmt.Errorf("RCON 未启用")
	}

	conn, err := net.DialTimeout("tcp", rconHost, 5*time.Second)
	if err != nil {
		return fmt.Errorf("连接 RCON 失败: %w", err)
	}
	defer conn.Close()

	// 认证
	if err := rconAuth(conn); err != nil {
		return err
	}

	// 发送命令
	return rconCmd(conn, cmd)
}

func rconAuth(conn net.Conn) error {
	packet := rconPacket(0, rconLogin, rconPassword)
	if _, err := conn.Write(packet); err != nil {
		return fmt.Errorf("发送认证包失败: %w", err)
	}

	// 读响应（需要读两次：requestID 响应 + 认证结果）
	resp := make([]byte, 4096)
	n, err := conn.Read(resp)
	if err != nil {
		return fmt.Errorf("读取认证响应失败: %w", err)
	}

	// 解析 requestID，如果匹配说明认证失败（返回 -1）
	if n >= 12 {
		reqID := int32(binary.LittleEndian.Uint32(resp[4:8]))
		if reqID == -1 {
			return fmt.Errorf("RCON 认证失败，密码错误")
		}
	}
	return nil
}

func rconCmd(conn net.Conn, cmd string) error {
	packet := rconPacket(1, rconCommand, cmd)
	if _, err := conn.Write(packet); err != nil {
		return fmt.Errorf("发送命令包失败: %w", err)
	}

	resp := make([]byte, 4096)
	n, err := conn.Read(resp)
	if err != nil {
		return fmt.Errorf("读取命令响应失败: %w", err)
	}

	if n > 12 {
		body := string(resp[12 : n-2]) // 跳过 length+id+type，去掉末尾 \x00\x00
		log.Printf("RCON 响应: %s", body)
	}
	return nil
}

func rconPacket(id int32, typ int32, payload string) []byte {
	payloadBytes := append([]byte(payload), 0, 0)
	length := int32(4 + 4 + len(payloadBytes)) // id(4) + type(4) + payload+nulls

	buf := make([]byte, 4+length)
	binary.LittleEndian.PutUint32(buf[0:4], uint32(length))
	binary.LittleEndian.PutUint32(buf[4:8], uint32(id))
	binary.LittleEndian.PutUint32(buf[8:12], uint32(typ))
	copy(buf[12:], payloadBytes)
	return buf
}

// rconPunish 根据惩罚级别执行对应的命令
func rconPunish(player string, lv punishLevel) {
	if !rconEnable {
		return
	}

	var cmd string
	switch lv.Action {
	case "warn":
		cmd = fmt.Sprintf("tell %s §c[警告] 你的消息已被拦截，请注意言行。信誉分过低将导致禁言或封禁。", player)
	case "mute":
		minutes := int(lv.MuteDur.Minutes())
		cmd = fmt.Sprintf("tell %s §c[禁言] 你已被禁言 %d 分钟。", player, minutes)
		// qexed 可能不支持 /mute，用 tell 代替；如需真正禁言需要查 qexed 指令
	case "kick":
		cmd = fmt.Sprintf("kick %s §c你的消息违反服务器规则，已被踢出。", player)
	case "ban":
		cmd = fmt.Sprintf("ban %s §c你的消息严重违反服务器规则，已被封禁。", player)
	default:
		return
	}

	if err := rconExecute(cmd); err != nil {
		log.Printf("RCON 执行失败 [%s → %s]: %v", player, lv.Action, err)
	} else {
		log.Printf("RCON 已执行 [%s → %s]: %s", player, lv.Action, cmd)
	}
}
