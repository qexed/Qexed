package main

import (
	"log"

	"github.com/gin-gonic/gin"
)

func main() {
	// 加载已有信誉数据
	repStore.load()

	gin.SetMode(gin.ReleaseMode)
	r := gin.New()
	r.Use(gin.Logger(), gin.Recovery())

	setupRoutes(r)

	log.Printf("╔══════════════════════════════════════════╗")
	log.Printf("║  文本审核服务 v2.0                        ║")
	log.Printf("║  引擎: 腾讯云 TMS（~100ms）               ║")
	log.Printf("║  监听: %s                          ║", listenAddr)
	log.Printf("║  RCON 执法: %v                            ║", rconEnable)
	log.Printf("╚══════════════════════════════════════════╝")

	if err := r.Run(listenAddr); err != nil {
		log.Fatalf("启动失败: %v", err)
	}
}
