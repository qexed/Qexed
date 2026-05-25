# Qexed Docker 镜像

从仓库根目录构建：

```sh
docker build -f docker/qexed/Dockerfile --target runtime-cpu -t qexed:cpu .
docker build -f docker/qexed/Dockerfile --target runtime-gpu -t qexed:gpu .
```

如果 Docker 构建环境经过企业代理或自签 CA，使用 BuildKit secret 注入 CA：

```sh
docker build -f docker/qexed/Dockerfile --target runtime-cpu -t qexed:cpu \
  --secret id=ca_cert,src=/path/to/ca.crt .
```

CPU 版本运行：

```sh
docker run --rm -it \
  -p 25565:25565 \
  -v qexed-config:/app/config \
  -v qexed-world:/app/world \
  -v qexed-logs:/app/logs \
  -v qexed-plugins:/app/plugins \
  -v qexed-resourcepacks:/app/resourcepacks \
  qexed:cpu
```

GPU 版本需要在 `config/qexed.toml` 中启用 `server.world.gpu.enable = true`。

NVIDIA：

```sh
docker run --rm -it --gpus all \
  -p 25565:25565 \
  -v qexed-config:/app/config \
  -v qexed-world:/app/world \
  -v qexed-logs:/app/logs \
  -v qexed-plugins:/app/plugins \
  -v qexed-resourcepacks:/app/resourcepacks \
  qexed:gpu
```

Intel/AMD：

```sh
docker run --rm -it --device /dev/dri \
  -p 25565:25565 \
  -v qexed-config:/app/config \
  -v qexed-world:/app/world \
  -v qexed-logs:/app/logs \
  -v qexed-plugins:/app/plugins \
  -v qexed-resourcepacks:/app/resourcepacks \
  qexed:gpu
```
