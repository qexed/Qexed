# Qexed Docker 镜像

从仓库根目录构建：

```sh
docker build -f docker/qexed/Dockerfile --target runtime-cpu -t qexed:latest .
docker build -f docker/qexed/Dockerfile --target runtime-gpu -t qexed:gpu .
```

如果 Docker 构建环境经过企业代理或自签 CA，使用 BuildKit secret 注入 CA：

```sh
docker build -f docker/qexed/Dockerfile --target runtime-cpu -t qexed:latest \
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
  qexed:latest
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

## 发布镜像

仓库提供 `.github/workflows/docker-publish.yml`。发布由 GitHub Actions 在云端完成，本地机器不需要直连 Docker Hub。

默认发布到 GitHub Container Registry：

```sh
ghcr.io/<owner>/<repo>:<branch>
ghcr.io/<owner>/<repo>:<branch>-gpu
ghcr.io/<owner>/<repo>:<tag>
ghcr.io/<owner>/<repo>:<tag>-gpu
ghcr.io/<owner>/<repo>:sha-<commit>
ghcr.io/<owner>/<repo>:sha-<commit>-gpu
```

只有严格匹配 `vX.Y.Z` 的正式 tag 会额外发布：

```sh
ghcr.io/<owner>/<repo>:latest
ghcr.io/<owner>/<repo>:latest-gpu
```

普通 commit、分支 push、`vX.Y.Z-rcN` 等非正式版本不会覆盖 `latest-*`。

GHCR 不需要额外 token。Actions 使用仓库自带的 `GITHUB_TOKEN`，但仓库权限需要允许 workflow 写入 packages：

```txt
Settings -> Actions -> General -> Workflow permissions -> Read and write permissions
```

如果还要同步到 Docker Hub，在 GitHub 仓库中配置：

```txt
Settings -> Secrets and variables -> Actions -> Variables
DOCKERHUB_IMAGE=<dockerhub-namespace>/<image-name>

Settings -> Secrets and variables -> Actions -> Secrets
DOCKERHUB_USERNAME=<dockerhub-username>
DOCKERHUB_TOKEN=<dockerhub-access-token>
```

没有配置这些值时，workflow 只发布 GHCR，不会尝试连接 Docker Hub。

手动发布：

```sh
gh workflow run "Publish Qexed Docker Images"
```

发布多架构镜像时手动传入 platforms：

```sh
gh workflow run "Publish Qexed Docker Images" -f platforms=linux/amd64,linux/arm64
```

当前默认只发布 `linux/amd64`，因为 GPU 运行库和 Rust 依赖在多架构上需要单独验证。
