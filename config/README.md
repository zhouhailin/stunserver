# 配置文件说明

本目录存放本地运行配置 `config/stunserver.conf`（TOML 格式），基于模板 [`deploy/stunserver.conf.example`](../deploy/stunserver.conf.example) 复制，并将路径改为相对路径（如 `log_dir = "log"`），相对于启动时的工作目录解析。模板本身保留绝对路径，用于安装到 `/opt/stunserver`。

## 配置从哪来

- **默认加载**：未传入 `--config` 时，服务自动读取工作目录下的 `config/stunserver.conf`；该文件不存在则使用内置默认值。
- **显式指定**：`--config <path>` 优先于默认路径；指定的文件不存在会直接报错退出。
- **安装部署**：`deploy/Makefile` / `Makefile` 会把 `deploy/stunserver.conf.example` 安装为 `/opt/stunserver/etc/stunserver.conf`，systemd 通过 `--config` 指向它；Docker 镜像同样使用容器内 `./etc/stunserver.conf`。
- **Docker 环境变量**：`docker/docker-entrypoint.sh` 使用 `STUN_CONFIG_PATH` 指定配置路径（默认 `/opt/stunserver/etc/stunserver.conf`）。

## 本地开发

```bash
# 从模板复制（首次或模板更新后），再把 log_dir 等路径改为相对路径
cp deploy/stunserver.conf.example config/stunserver.conf

# 在项目根目录直接启动，自动加载 config/stunserver.conf，日志写入 ./log/
cargo run --bin stunserver

# 或显式指定其他配置
cargo run --bin stunserver -- --config config/stunserver.conf
```

`config/stunserver.conf` 属于本地自定义配置（已加入 `.gitignore`），修改后不要提交；模板变更时请同步更新 `deploy/stunserver.conf.example`。
