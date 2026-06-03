## 发布镜像

仓库提供 `.github/workflows/docker-publish.yml`。发布由 GitHub Actions 在云端完成，本地机器不需要直连 Docker Hub。

默认发布到 GitHub Container Registry：

```sh
ghcr.io/<owner>/<repo>:<branch>
ghcr.io/<owner>/<repo>:<tag>
ghcr.io/<owner>/<repo>:sha-<commit>
```

只有严格匹配 `vX.Y.Z` 的正式 tag 会额外发布：

```sh
ghcr.io/<owner>/<repo>:latest
```

普通 commit、分支 push、`vX.Y.Z-rcN` 等非正式版本不会覆盖 `latest`。

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

当前默认只发布 linux/amd64。
