mod formatter;
pub use formatter::{config_format_label, format_and_validate_config};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn to_slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// 返回某服务对应的主配置文件路径
pub fn service_config_path(
    config_kind: Option<&str>,
    app_dir: &Path,
    conf_dir: &Path,
    data_dir: &Path,
) -> Option<PathBuf> {
    match config_kind {
        Some("mysql") => Some(conf_dir.join("my.ini")),
        Some("redis") => Some(conf_dir.join("redis.conf")),
        Some("nginx") => Some(conf_dir.join("nginx.conf")),
        Some("nacos") => Some(app_dir.join("conf").join("application.properties")),
        Some("postgres") => Some(data_dir.join("postgresql.conf")),
        Some("tomcat") => Some(app_dir.join("conf").join("server.xml")),
        Some("mongodb") => Some(conf_dir.join("mongod.cfg")),
        Some("mailpit") => Some(conf_dir.join("mailpit.env")),
        _ => None,
    }
}

/// 生成默认配置文本
pub fn default_service_config_content(
    config_kind: Option<&str>,
    app_dir: &Path,
    data_dir: &Path,
    port: u16,
) -> Option<String> {
    match config_kind {
        Some("mysql") => Some(format!(
            "[mysqld]\n\
             port={port}\n\
             bind-address=127.0.0.1\n\
             basedir={}\n\
             datadir={}\n\
             character-set-server=utf8mb4\n\
             collation-server=utf8mb4_unicode_ci\n\
             default-time-zone='+8:00'\n\
             default-storage-engine=INNODB\n\
             max_connections=200\n\
             innodb_buffer_pool_size=256M\n\
             max_allowed_packet=64M\n\
             slow_query_log=0\n\
             long_query_time=2\n\
             \n\
             [client]\n\
             port={port}\n\
             default-character-set=utf8mb4\n",
            to_slash(app_dir),
            to_slash(data_dir),
        )),
        Some("redis") => {
            let folder_name = data_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "redis-7.2".to_string());
            Some(format!(
                "bind 127.0.0.1\n\
                 port {port}\n\
                 protected-mode yes\n\
                 databases 16\n\
                 timeout 0\n\
                 loglevel notice\n\
                 dir \"../../data/{folder_name}\"\n\
                 dbfilename dump.rdb\n\
                 appendonly yes\n\
                 appendfsync everysec\n\
                 maxmemory 256mb\n\
                 maxmemory-policy allkeys-lru\n"
            ))
        }
        Some("nginx") => {
            let html_dir = data_dir.join("html");
            let mime_types = to_slash(&app_dir.join("conf").join("mime.types"));
            Some(format!(
                "worker_processes  1;\n\
                 events {{\n\
                     worker_connections  1024;\n\
                 }}\n\
                 http {{\n\
                     include       \"{mime_types}\";\n\
                     default_type  application/octet-stream;\n\
                     sendfile      on;\n\
                     keepalive_timeout 65;\n\
                     client_max_body_size 50m;\n\
                     gzip on;\n\
                     server {{\n\
                         listen       {port};\n\
                         server_name  localhost;\n\
                         location / {{\n\
                             root   \"{}\";\n\
                             index  index.html index.htm;\n\
                         }}\n\
                     }}\n\
                 }}\n",
                to_slash(&html_dir),
            ))
        }
        Some("nacos") => Some(format!(
            "### Moka Nacos 默认配置 ###\n\
             server.servlet.contextPath=/nacos\n\
             server.port={port}\n\
             \n\
             ### 鉴权配置（默认关闭，开启后请保留 32 位以上 Base64 Secret Key）###\n\
             nacos.core.auth.system.type=nacos\n\
             nacos.core.auth.enabled=false\n\
             nacos.core.auth.plugin.nacos.token.secret.key=VGhpc0lzTXlDdXN0b21TZWNyZXRLZXkwMTIzNDU2Nzg=\n\
             nacos.core.auth.server.identity.key=moka-identity\n\
             nacos.core.auth.server.identity.value=moka-secret\n\
             \n\
             ### 数据源配置（默认使用内置 Derby 单机存储；如需切换外置 MySQL，请取消以下注释）###\n\
             # spring.sql.init.platform=mysql\n\
             # db.num=1\n\
             # db.url.0=jdbc:mysql://127.0.0.1:3306/nacos?characterEncoding=utf8&connectTimeout=1000&socketTimeout=3000&autoReconnect=true&useUnicode=true&useSSL=false&serverTimezone=UTC\n\
             # db.user.0=root\n\
             # db.password.0=\n"
        )),
        Some("postgres") => Some(format!(
            "# Moka PostgreSQL 默认配置\n\
             listen_addresses = 'localhost'\n\
             port = {port}\n\
             max_connections = 100\n\
             shared_buffers = 128MB\n\
             work_mem = 4MB\n\
             timezone = 'Asia/Shanghai'\n\
             log_statement = 'none'\n"
        )),
        Some("tomcat") => Some(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <Server port=\"8005\" shutdown=\"SHUTDOWN\">\n\
               <Listener className=\"org.apache.catalina.startup.VersionLoggerListener\" />\n\
               <Listener className=\"org.apache.catalina.core.JreMemoryLeakPreventionListener\" />\n\
               <Service name=\"Catalina\">\n\
                 <Connector port=\"{port}\" protocol=\"HTTP/1.1\"\n\
                            connectionTimeout=\"20000\"\n\
                            maxThreads=\"200\"\n\
                            URIEncoding=\"UTF-8\"\n\
                            redirectPort=\"8443\" />\n\
                 <Engine name=\"Catalina\" defaultHost=\"localhost\">\n\
                   <Host name=\"localhost\" appBase=\"webapps\" unpackWARs=\"true\" autoDeploy=\"true\" />\n\
                 </Engine>\n\
               </Service>\n\
             </Server>\n"
        )),
        Some("mongodb") => Some(format!(
            "# Moka MongoDB 默认配置\nstorage:\n  dbPath: \"{}\"\nnet:\n  bindIp: 127.0.0.1\n  port: {port}\nsecurity:\n  authorization: disabled\n",
            to_slash(data_dir),
        )),
        Some("mailpit") => Some(format!(
            "# Moka Mailpit 本地邮件捕获器配置\n\
             MP_UI_BIND_ADDR=127.0.0.1:{port}\n\
             MP_SMTP_BIND_ADDR=127.0.0.1:1025\n\
             MP_MAX_MESSAGES=500\n\
             MP_SMTP_AUTH_ACCEPT_ANY=1\n\
             MP_SMTP_AUTH_ALLOW_INSECURE=1\n"
        )),
        _ => None,
    }
}

/// 确保服务配置文件存在，并在端口变更时同步更新已有配置文件中的端口项
pub fn ensure_service_config(
    config_kind: Option<&str>,
    app_dir: &Path,
    conf_dir: &Path,
    data_dir: &Path,
    port: u16,
) -> Result<Option<PathBuf>> {
    fs::create_dir_all(conf_dir)?;
    fs::create_dir_all(data_dir)?;

    if config_kind == Some("nginx") {
        let html_dir = data_dir.join("html");
        fs::create_dir_all(&html_dir)?;
        let index_html = html_dir.join("index.html");
        if !index_html.exists() {
            fs::write(
                &index_html,
                "<!doctype html><html><head><meta charset='utf-8'><title>Moka Nginx</title></head>\
                 <body style='font-family:sans-serif;padding:40px'><h1>Moka Nginx 运行中</h1>\
                 <p>你可以在 Moka 的可视化配置面板中自定义站点、上传限制与反向代理规则。</p></body></html>",
            )?;
        }
    }

    let Some(path) = service_config_path(config_kind, app_dir, conf_dir, data_dir) else {
        return Ok(None);
    };

    // PostgreSQL 的 postgresql.conf 由首次启动 initdb 自动生成，未初始化前不要写入文件以免阻塞 initdb
    if config_kind == Some("postgres") && !path.exists() {
        return Ok(None);
    }

    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(content) = default_service_config_content(config_kind, app_dir, data_dir, port)
        {
            fs::write(&path, content)?;
        }
    } else if port > 0 {
        if let Ok(old) = fs::read_to_string(&path) {
            let updated = sync_port_in_config(config_kind, &old, port);
            if updated != old {
                let _ = fs::write(&path, updated);
            }
        }
    }

    Ok(Some(path))
}

/// 读取服务配置文件中当前生效的端口（与可视化表单展示的值保持同一套解析）
pub fn read_service_port(
    config_kind: Option<&str>,
    app_dir: &Path,
    conf_dir: &Path,
    data_dir: &Path,
    default_port: u16,
) -> Option<u16> {
    let path = service_config_path(config_kind, app_dir, conf_dir, data_dir)?;
    let content = fs::read_to_string(path).ok()?;
    let port = match parse_visual_config(config_kind, &content, default_port)? {
        VisualConfig::Mysql { port, .. }
        | VisualConfig::Redis { port, .. }
        | VisualConfig::Nginx { port, .. }
        | VisualConfig::Nacos { port, .. }
        | VisualConfig::Postgres { port, .. }
        | VisualConfig::Tomcat { port, .. }
        | VisualConfig::MongoDb { port, .. }
        | VisualConfig::Mailpit { port, .. } => port,
    };
    port.trim().parse::<u16>().ok().filter(|p| *p > 0)
}

fn replace_nginx_listen_port(line: &str, new_port: u16) -> String {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let body = line.trim().trim_end_matches(';').trim();
    let Some(rest) = body.strip_prefix("listen") else {
        return line.to_string();
    };
    let rest = rest.trim();
    let mut tokens = rest.split_whitespace();
    let Some(first) = tokens.next() else {
        return line.to_string();
    };
    let tail: Vec<&str> = tokens.collect();
    let new_first = if let Some((host, _old_port)) = first.rsplit_once(':') {
        format!("{host}:{new_port}")
    } else if first.chars().all(|c| c.is_ascii_digit()) {
        new_port.to_string()
    } else {
        return line.to_string();
    };
    if tail.is_empty() {
        format!("{indent}listen       {new_first};")
    } else {
        format!("{indent}listen       {new_first} {};", tail.join(" "))
    }
}

fn sync_port_in_config(config_kind: Option<&str>, content: &str, port: u16) -> String {
    if config_kind == Some("tomcat") {
        return update_xml_connector_attr(content, "port", &port.to_string());
    }
    let mut nginx_listen_updated = false;
    let mut lines: Vec<String> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        let replaced = match config_kind {
            Some("mysql") if trimmed.starts_with("port=") => format!("port={port}"),
            Some("redis") if trimmed.starts_with("port ") => format!("port {port}"),
            Some("nginx")
                if !nginx_listen_updated
                    && trimmed.starts_with("listen ")
                    && trimmed.ends_with(';')
                    && !trimmed.contains("ssl") =>
            {
                nginx_listen_updated = true;
                replace_nginx_listen_port(line, port)
            }
            Some("nacos") if trimmed.starts_with("server.port=") => format!("server.port={port}"),
            Some("postgres")
                if trimmed.starts_with("port =") || trimmed.starts_with("port=") =>
            {
                format!("port = {port}")
            }
            Some("mongodb") if trimmed.starts_with("port:") => {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                format!("{indent}port: {port}")
            }
            Some("mailpit") if trimmed.starts_with("MP_UI_BIND_ADDR=") => {
                let host = trimmed
                    .strip_prefix("MP_UI_BIND_ADDR=")
                    .and_then(|v| v.rsplit_once(':'))
                    .map(|(h, _)| h)
                    .unwrap_or("127.0.0.1");
                format!("MP_UI_BIND_ADDR={host}:{port}")
            }
            _ => line.to_string(),
        };
        lines.push(replaced);
    }
    let mut out = lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[derive(Debug, Clone)]
pub struct NginxProxyRule {
    pub prefix: String,
    pub target: String,
    pub cors: bool,
    pub websocket: bool,
}

/// 面向小白用户的完整分组可视化配置
#[derive(Debug, Clone)]
pub enum VisualConfig {
    Mysql {
        port: String,
        bind_address: String,
        charset: String,
        time_zone: String,
        relaxed_sql_mode: bool,
        max_connections: String,
        innodb_buffer_pool_size: String,
        max_allowed_packet: String,
        slow_query_log: bool,
        long_query_time: String,
    },
    Redis {
        port: String,
        bind: String,
        password: String,
        protected_mode: bool,
        databases: String,
        maxmemory: String,
        maxmemory_policy: String,
        appendonly: bool,
        appendfsync: String,
        timeout: String,
        loglevel: String,
    },
    Nginx {
        worker_processes: String,
        worker_connections: String,
        client_max_body_size: String,
        gzip: bool,
        port: String,
        server_name: String,
        root_dir: String,
        proxy_rules: Vec<NginxProxyRule>,
        ssl_enabled: bool,
        ssl_port: String,
        ssl_cert: String,
        ssl_key: String,
    },
    Nacos {
        port: String,
        context_path: String,
        auth_enabled: bool,
        auth_secret_key: String,
        use_mysql: bool,
        mysql_url: String,
        mysql_user: String,
        mysql_password: String,
    },
    Postgres {
        port: String,
        listen_addresses: String,
        max_connections: String,
        shared_buffers: String,
        work_mem: String,
        timezone: String,
        log_statement: String,
    },
    Tomcat {
        port: String,
        shutdown_port: String,
        uri_encoding: String,
        max_threads: String,
        connection_timeout: String,
    },
    MongoDb {
        port: String,
        bind_ip: String,
        db_path: String,
        auth_enabled: bool,
    },
    Mailpit {
        port: String,
        smtp_port: String,
        bind_host: String,
        max_messages: String,
        accept_any_auth: bool,
    },
}

/// 从原始配置文本解析可视化字段
pub fn parse_visual_config(
    config_kind: Option<&str>,
    content: &str,
    default_port: u16,
) -> Option<VisualConfig> {
    match config_kind {
        Some("mysql") => {
            let mut port = default_port.to_string();
            let mut bind_address = "127.0.0.1".to_string();
            let mut charset = "utf8mb4".to_string();
            let mut time_zone = "+8:00".to_string();
            let mut relaxed_sql_mode = false;
            let mut max_connections = "200".to_string();
            let mut innodb_buffer_pool_size = "256M".to_string();
            let mut max_allowed_packet = "64M".to_string();
            let mut slow_query_log = false;
            let mut long_query_time = "2".to_string();

            for line in content.lines() {
                let t = line.trim();
                if t.starts_with('#') || t.is_empty() {
                    continue;
                }
                if let Some(v) = t.strip_prefix("port=") {
                    port = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("bind-address=") {
                    bind_address = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("character-set-server=") {
                    charset = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("default-time-zone=") {
                    time_zone = v.trim().trim_matches('\'').trim_matches('"').to_string();
                } else if let Some(v) = t.strip_prefix("sql_mode=") {
                    relaxed_sql_mode = !v.to_uppercase().contains("ONLY_FULL_GROUP_BY");
                } else if let Some(v) = t.strip_prefix("max_connections=") {
                    max_connections = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("innodb_buffer_pool_size=") {
                    innodb_buffer_pool_size = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("max_allowed_packet=") {
                    max_allowed_packet = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("slow_query_log=") {
                    slow_query_log = v.trim() == "1" || v.trim().eq_ignore_ascii_case("ON");
                } else if let Some(v) = t.strip_prefix("long_query_time=") {
                    long_query_time = v.trim().to_string();
                }
            }

            Some(VisualConfig::Mysql {
                port,
                bind_address,
                charset,
                time_zone,
                relaxed_sql_mode,
                max_connections,
                innodb_buffer_pool_size,
                max_allowed_packet,
                slow_query_log,
                long_query_time,
            })
        }
        Some("redis") => {
            let mut port = default_port.to_string();
            let mut bind = "127.0.0.1".to_string();
            let mut password = String::new();
            let mut protected_mode = true;
            let mut databases = "16".to_string();
            let mut maxmemory = "256mb".to_string();
            let mut maxmemory_policy = "allkeys-lru".to_string();
            let mut appendonly = true;
            let mut appendfsync = "everysec".to_string();
            let mut timeout = "0".to_string();
            let mut loglevel = "notice".to_string();

            for line in content.lines() {
                let t = line.trim();
                if t.starts_with('#') || t.is_empty() {
                    continue;
                }
                if let Some(v) = t.strip_prefix("port ") {
                    port = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("bind ") {
                    bind = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("requirepass ") {
                    password = v.trim().trim_matches('"').to_string();
                } else if let Some(v) = t.strip_prefix("protected-mode ") {
                    protected_mode = v.trim().eq_ignore_ascii_case("yes");
                } else if let Some(v) = t.strip_prefix("databases ") {
                    databases = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("maxmemory ") {
                    maxmemory = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("maxmemory-policy ") {
                    maxmemory_policy = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("appendonly ") {
                    appendonly = v.trim().eq_ignore_ascii_case("yes");
                } else if let Some(v) = t.strip_prefix("appendfsync ") {
                    appendfsync = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("timeout ") {
                    timeout = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("loglevel ") {
                    loglevel = v.trim().to_string();
                }
            }

            Some(VisualConfig::Redis {
                port,
                bind,
                password,
                protected_mode,
                databases,
                maxmemory,
                maxmemory_policy,
                appendonly,
                appendfsync,
                timeout,
                loglevel,
            })
        }
        Some("nginx") => {
            let mut worker_processes = "1".to_string();
            let mut worker_connections = "1024".to_string();
            let mut client_max_body_size = "50m".to_string();
            let mut gzip = true;
            let mut port = default_port.to_string();
            let mut server_name = "localhost".to_string();
            let mut root_dir = String::new();
            let mut proxy_rules: Vec<NginxProxyRule> = Vec::new();
            let mut cur_rule: Option<NginxProxyRule> = None;
            let mut ssl_enabled = false;
            let mut ssl_port = "443".to_string();
            let mut ssl_cert = String::new();
            let mut ssl_key = String::new();

            let mut in_moka_proxy = false;
            let mut in_moka_ssl = false;
            for line in content.lines() {
                let t = line.trim();
                if t == "# MOKA_SSL_BEGIN" {
                    in_moka_ssl = true;
                    ssl_enabled = true;
                    continue;
                }
                if t == "# MOKA_SSL_END" {
                    in_moka_ssl = false;
                    continue;
                }
                if in_moka_ssl {
                    if let Some(rest) = t.strip_prefix("listen ") {
                        if let Some(pt) = rest.split_whitespace().next() {
                            ssl_port = pt.trim_end_matches(';').to_string();
                        }
                    } else if let Some(rest) = t.strip_prefix("ssl_certificate ") {
                        ssl_cert = rest
                            .trim()
                            .trim_end_matches(';')
                            .trim()
                            .trim_matches('"')
                            .to_string();
                    } else if let Some(rest) = t.strip_prefix("ssl_certificate_key ") {
                        ssl_key = rest
                            .trim()
                            .trim_end_matches(';')
                            .trim()
                            .trim_matches('"')
                            .to_string();
                    }
                    continue;
                }

                if t == "# MOKA_PROXY_BEGIN" {
                    in_moka_proxy = true;
                    continue;
                }
                if t == "# MOKA_PROXY_END" {
                    if let Some(r) = cur_rule.take() {
                        proxy_rules.push(r);
                    }
                    in_moka_proxy = false;
                    continue;
                }
                if in_moka_proxy {
                    if let Some(rest) = t.strip_prefix("location ") {
                        if let Some(prev) = cur_rule.take() {
                            proxy_rules.push(prev);
                        }
                        cur_rule = Some(NginxProxyRule {
                            prefix: rest.trim().trim_end_matches('{').trim().to_string(),
                            target: "http://127.0.0.1:8080".to_string(),
                            cors: false,
                            websocket: false,
                        });
                    } else if let Some(rule) = cur_rule.as_mut() {
                        if let Some(rest) = t.strip_prefix("proxy_pass ") {
                            rule.target = rest.trim().trim_end_matches(';').trim().to_string();
                        } else if t.contains("Access-Control-Allow-Origin") {
                            rule.cors = true;
                        } else if t.contains("Upgrade $http_upgrade") {
                            rule.websocket = true;
                        }
                    }
                    continue;
                }

                if let Some(v) = t.strip_prefix("worker_processes ") {
                    worker_processes = v.trim().trim_end_matches(';').trim().to_string();
                } else if let Some(v) = t.strip_prefix("worker_connections ") {
                    worker_connections = v.trim().trim_end_matches(';').trim().to_string();
                } else if let Some(v) = t.strip_prefix("client_max_body_size ") {
                    client_max_body_size = v.trim().trim_end_matches(';').trim().to_string();
                } else if let Some(v) = t.strip_prefix("gzip ") {
                    gzip = v.trim().trim_end_matches(';').trim().eq_ignore_ascii_case("on");
                } else if let Some(v) = t.strip_prefix("listen ") {
                    if !v.contains("ssl") {
                        port = v.trim().trim_end_matches(';').trim().to_string();
                    }
                } else if let Some(v) = t.strip_prefix("server_name ") {
                    server_name = v.trim().trim_end_matches(';').trim().to_string();
                } else if let Some(v) = t.strip_prefix("root ") {
                    root_dir = v
                        .trim()
                        .trim_end_matches(';')
                        .trim()
                        .trim_matches('"')
                        .to_string();
                }
            }

            Some(VisualConfig::Nginx {
                worker_processes,
                worker_connections,
                client_max_body_size,
                gzip,
                port,
                server_name,
                root_dir,
                proxy_rules,
                ssl_enabled,
                ssl_port,
                ssl_cert,
                ssl_key,
            })
        }
        Some("nacos") => {
            let mut port = default_port.to_string();
            let mut context_path = "/nacos".to_string();
            let mut auth_enabled = false;
            let mut auth_secret_key =
                "VGhpc0lzTXlDdXN0b21TZWNyZXRLZXkwMTIzNDU2Nzg=".to_string();
            let mut use_mysql = false;
            let mut mysql_url = "jdbc:mysql://127.0.0.1:3306/nacos?characterEncoding=utf8&connectTimeout=1000&socketTimeout=3000&autoReconnect=true&useUnicode=true&useSSL=false&serverTimezone=UTC".to_string();
            let mut mysql_user = "root".to_string();
            let mut mysql_password = String::new();

            for line in content.lines() {
                let t = line.trim();
                if t.starts_with('#') || t.is_empty() {
                    continue;
                }
                if let Some((k, v)) = t.split_once('=') {
                    let key = k.trim();
                    let val = v.trim();
                    match key {
                        "server.port" => port = val.to_string(),
                        "server.servlet.contextPath" => context_path = val.to_string(),
                        "nacos.core.auth.enabled" => {
                            auth_enabled = val.eq_ignore_ascii_case("true")
                        }
                        "nacos.core.auth.plugin.nacos.token.secret.key" if !val.is_empty() => {
                            auth_secret_key = val.to_string()
                        }
                        "spring.sql.init.platform" | "spring.datasource.platform" => {
                            use_mysql = val.eq_ignore_ascii_case("mysql")
                        }
                        "db.url.0" if !val.is_empty() => mysql_url = val.to_string(),
                        "db.user.0" if !val.is_empty() => mysql_user = val.to_string(),
                        "db.password.0" => mysql_password = val.to_string(),
                        _ => {}
                    }
                }
            }

            Some(VisualConfig::Nacos {
                port,
                context_path,
                auth_enabled,
                auth_secret_key,
                use_mysql,
                mysql_url,
                mysql_user,
                mysql_password,
            })
        }
        Some("postgres") => {
            let mut port = default_port.to_string();
            let mut listen_addresses = "localhost".to_string();
            let mut max_connections = "100".to_string();
            let mut shared_buffers = "128MB".to_string();
            let mut work_mem = "4MB".to_string();
            let mut timezone = "Asia/Shanghai".to_string();
            let mut log_statement = "none".to_string();

            for line in content.lines() {
                let t = line.trim();
                if t.starts_with('#') || t.is_empty() {
                    continue;
                }
                // 去掉行尾注释
                let effective = t.split('#').next().unwrap_or(t).trim();
                if let Some((k, v)) = effective.split_once('=') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('\'').trim_matches('"').to_string();
                    match key {
                        "port" => port = val,
                        "listen_addresses" => listen_addresses = val,
                        "max_connections" => max_connections = val,
                        "shared_buffers" => shared_buffers = val,
                        "work_mem" => work_mem = val,
                        "timezone" => timezone = val,
                        "log_statement" => log_statement = val,
                        _ => {}
                    }
                }
            }

            Some(VisualConfig::Postgres {
                port,
                listen_addresses,
                max_connections,
                shared_buffers,
                work_mem,
                timezone,
                log_statement,
            })
        }
        Some("tomcat") => {
            let connector_block = find_http_connector_slice(content).unwrap_or("");
            let port = extract_xml_attr(connector_block, "port")
                .unwrap_or_else(|| default_port.to_string());
            let uri_encoding = extract_xml_attr(connector_block, "URIEncoding")
                .unwrap_or_else(|| "UTF-8".to_string());
            let max_threads = extract_xml_attr(connector_block, "maxThreads")
                .unwrap_or_else(|| "200".to_string());
            let connection_timeout = extract_xml_attr(connector_block, "connectionTimeout")
                .unwrap_or_else(|| "20000".to_string());
            let shutdown_port = find_server_tag_slice(content)
                .and_then(|s| extract_xml_attr(s, "port"))
                .unwrap_or_else(|| "8005".to_string());

            Some(VisualConfig::Tomcat {
                port,
                shutdown_port,
                uri_encoding,
                max_threads,
                connection_timeout,
            })
        }
        Some("mongodb") => {
            let mut port = default_port.to_string();
            let mut bind_ip = "127.0.0.1".to_string();
            let mut db_path = String::new();
            let mut auth_enabled = false;
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with('#') || t.is_empty() {
                    continue;
                }
                if let Some(v) = t.strip_prefix("port:") {
                    port = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("bindIp:") {
                    bind_ip = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("dbPath:") {
                    db_path = v.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(v) = t.strip_prefix("authorization:") {
                    auth_enabled = v.trim().eq_ignore_ascii_case("enabled");
                }
            }
            Some(VisualConfig::MongoDb {
                port,
                bind_ip,
                db_path,
                auth_enabled,
            })
        }
        Some("mailpit") => {
            let mut port = default_port.to_string();
            let mut smtp_port = "1025".to_string();
            let mut bind_host = "127.0.0.1".to_string();
            let mut max_messages = "500".to_string();
            let mut accept_any_auth = true;
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with('#') || t.is_empty() {
                    continue;
                }
                if let Some((k, v)) = t.split_once('=') {
                    let val = v.trim();
                    match k.trim() {
                        "MP_UI_BIND_ADDR" => {
                            if let Some((h, p)) = val.rsplit_once(':') {
                                if !h.is_empty() {
                                    bind_host = h.to_string();
                                }
                                port = p.to_string();
                            }
                        }
                        "MP_SMTP_BIND_ADDR" => {
                            if let Some((_, p)) = val.rsplit_once(':') {
                                smtp_port = p.to_string();
                            }
                        }
                        "MP_MAX_MESSAGES" => max_messages = val.to_string(),
                        "MP_SMTP_AUTH_ACCEPT_ANY" => {
                            accept_any_auth = val == "1" || val.eq_ignore_ascii_case("true");
                        }
                        _ => {}
                    }
                }
            }
            Some(VisualConfig::Mailpit {
                port,
                smtp_port,
                bind_host,
                max_messages,
                accept_any_auth,
            })
        }
        _ => None,
    }
}

/// 辅助：在 `[mysqld]` 段中更新或插入键值
fn upsert_ini_in_section(lines: &mut Vec<String>, section: &str, key: &str, value: &str) {
    let prefix = format!("{key}=");
    let mut in_section = false;
    let mut insert_pos: Option<usize> = None;

    for (i, line) in lines.iter_mut().enumerate() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            if in_section && insert_pos.is_none() {
                insert_pos = Some(i);
            }
            in_section = t.eq_ignore_ascii_case(section);
            continue;
        }
        if in_section && t.starts_with(&prefix) {
            *line = format!("{key}={value}");
            return;
        }
    }

    if let Some(pos) = insert_pos {
        lines.insert(pos, format!("{key}={value}"));
    } else {
        lines.push(format!("{key}={value}"));
    }
}

fn remove_ini_key(lines: &mut Vec<String>, key: &str) {
    let prefix = format!("{key}=");
    lines.retain(|l| !l.trim().starts_with(&prefix));
}

fn upsert_space_kv(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    let prefix = format!("{key} ");
    let mut found = false;
    let mut out = Vec::with_capacity(lines.len() + 1);
    for line in lines.drain(..) {
        if line.trim().starts_with(&prefix) {
            found = true;
            if let Some(v) = value {
                out.push(format!("{key} {v}"));
            }
        } else {
            out.push(line);
        }
    }
    if !found {
        if let Some(v) = value {
            out.push(format!("{key} {v}"));
        }
    }
    *lines = out;
}

/// 将可视化表单的修改安全写回原始配置文本（保留其余自定义配置项）
pub fn apply_visual_config(content: &str, cfg: &VisualConfig) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    match cfg {
        VisualConfig::Mysql {
            port,
            bind_address,
            charset,
            time_zone,
            relaxed_sql_mode,
            max_connections,
            innodb_buffer_pool_size,
            max_allowed_packet,
            slow_query_log,
            long_query_time,
        } => {
            // port 同时更新 [mysqld] 与 [client]
            for line in &mut lines {
                if line.trim().starts_with("port=") {
                    *line = format!("port={}", port.trim());
                }
            }
            upsert_ini_in_section(&mut lines, "[mysqld]", "bind-address", bind_address.trim());
            upsert_ini_in_section(
                &mut lines,
                "[mysqld]",
                "character-set-server",
                charset.trim(),
            );
            upsert_ini_in_section(
                &mut lines,
                "[mysqld]",
                "default-time-zone",
                &format!("'{}'", time_zone.trim()),
            );
            if *relaxed_sql_mode {
                upsert_ini_in_section(
                    &mut lines,
                    "[mysqld]",
                    "sql_mode",
                    "NO_ENGINE_SUBSTITUTION",
                );
            } else {
                remove_ini_key(&mut lines, "sql_mode");
            }
            upsert_ini_in_section(
                &mut lines,
                "[mysqld]",
                "max_connections",
                max_connections.trim(),
            );
            upsert_ini_in_section(
                &mut lines,
                "[mysqld]",
                "innodb_buffer_pool_size",
                innodb_buffer_pool_size.trim(),
            );
            upsert_ini_in_section(
                &mut lines,
                "[mysqld]",
                "max_allowed_packet",
                max_allowed_packet.trim(),
            );
            upsert_ini_in_section(
                &mut lines,
                "[mysqld]",
                "slow_query_log",
                if *slow_query_log { "1" } else { "0" },
            );
            upsert_ini_in_section(
                &mut lines,
                "[mysqld]",
                "long_query_time",
                long_query_time.trim(),
            );
        }
        VisualConfig::Redis {
            port,
            bind,
            password,
            protected_mode,
            databases,
            maxmemory,
            maxmemory_policy,
            appendonly,
            appendfsync,
            timeout,
            loglevel,
        } => {
            upsert_space_kv(&mut lines, "port", Some(port.trim()));
            upsert_space_kv(&mut lines, "bind", Some(bind.trim()));
            let pass_opt = if password.trim().is_empty() {
                None
            } else {
                Some(password.trim())
            };
            upsert_space_kv(&mut lines, "requirepass", pass_opt);
            upsert_space_kv(
                &mut lines,
                "protected-mode",
                Some(if *protected_mode { "yes" } else { "no" }),
            );
            upsert_space_kv(&mut lines, "databases", Some(databases.trim()));
            upsert_space_kv(&mut lines, "maxmemory", Some(maxmemory.trim()));
            upsert_space_kv(&mut lines, "maxmemory-policy", Some(maxmemory_policy.trim()));
            upsert_space_kv(
                &mut lines,
                "appendonly",
                Some(if *appendonly { "yes" } else { "no" }),
            );
            upsert_space_kv(&mut lines, "appendfsync", Some(appendfsync.trim()));
            upsert_space_kv(&mut lines, "timeout", Some(timeout.trim()));
            upsert_space_kv(&mut lines, "loglevel", Some(loglevel.trim()));
        }
        VisualConfig::Nginx {
            worker_processes,
            worker_connections,
            client_max_body_size,
            gzip,
            port,
            server_name,
            root_dir,
            proxy_rules,
            ssl_enabled,
            ssl_port,
            ssl_cert,
            ssl_key,
        } => {
            // 先移除旧的 MOKA_PROXY 与 MOKA_SSL 块
            let mut cleaned = Vec::with_capacity(lines.len());
            let mut skip_proxy = false;
            let mut skip_ssl = false;
            for line in lines {
                let t = line.trim();
                if t == "# MOKA_PROXY_BEGIN" {
                    skip_proxy = true;
                    continue;
                }
                if t == "# MOKA_PROXY_END" {
                    skip_proxy = false;
                    continue;
                }
                if t == "# MOKA_SSL_BEGIN" {
                    skip_ssl = true;
                    continue;
                }
                if t == "# MOKA_SSL_END" {
                    skip_ssl = false;
                    continue;
                }
                if !skip_proxy && !skip_ssl {
                    cleaned.push(line);
                }
            }
            lines = cleaned;

            let mut has_body_size = false;
            let mut has_gzip = false;
            for line in &mut lines {
                let t = line.trim();
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                if t.starts_with("worker_processes ") && t.ends_with(';') {
                    *line = format!("{indent}worker_processes  {};", worker_processes.trim());
                } else if t.starts_with("worker_connections ") && t.ends_with(';') {
                    *line = format!("{indent}worker_connections  {};", worker_connections.trim());
                } else if t.starts_with("client_max_body_size ") && t.ends_with(';') {
                    *line = format!(
                        "{indent}client_max_body_size {};",
                        client_max_body_size.trim()
                    );
                    has_body_size = true;
                } else if t.starts_with("gzip ") && t.ends_with(';') {
                    *line = format!("{indent}gzip {};", if *gzip { "on" } else { "off" });
                    has_gzip = true;
                } else if t.starts_with("listen ") && t.ends_with(';') && !t.contains("ssl") {
                    *line = format!("{indent}listen       {};", port.trim());
                } else if t.starts_with("server_name ") && t.ends_with(';') {
                    *line = format!("{indent}server_name  {};", server_name.trim());
                } else if t.starts_with("root ") && t.ends_with(';') && !root_dir.trim().is_empty()
                {
                    let norm = root_dir.trim().replace('\\', "/");
                    *line = format!("{indent}root   \"{norm}\";");
                }
            }

            // 如旧配置里没有 client_max_body_size / gzip，则插入到 http { 内
            if !has_body_size || !has_gzip {
                if let Some(pos) = lines.iter().position(|l| l.trim().starts_with("keepalive_timeout")) {
                    let mut extra = Vec::new();
                    if !has_body_size {
                        extra.push(format!(
                            "    client_max_body_size {};",
                            client_max_body_size.trim()
                        ));
                    }
                    if !has_gzip {
                        extra.push(format!("    gzip {};", if *gzip { "on" } else { "off" }));
                    }
                    for (offset, item) in extra.into_iter().enumerate() {
                        lines.insert(pos + 1 + offset, item);
                    }
                }
            }

            // 如开启本地 HTTPS，在 server_name 行后插入 MOKA_SSL 块
            if *ssl_enabled && !ssl_cert.trim().is_empty() && !ssl_key.trim().is_empty() {
                let sp = if ssl_port.trim().is_empty() {
                    "443"
                } else {
                    ssl_port.trim()
                };
                let cert_norm = ssl_cert.trim().replace('\\', "/");
                let key_norm = ssl_key.trim().replace('\\', "/");
                let ssl_block = vec![
                    "        # MOKA_SSL_BEGIN".to_string(),
                    format!("        listen       {sp} ssl;"),
                    format!("        ssl_certificate     \"{cert_norm}\";"),
                    format!("        ssl_certificate_key \"{key_norm}\";"),
                    "        ssl_protocols       TLSv1.2 TLSv1.3;".to_string(),
                    "        # MOKA_SSL_END".to_string(),
                ];
                if let Some(pos) = lines.iter().position(|l| l.trim().starts_with("server_name ")) {
                    for (offset, item) in ssl_block.into_iter().enumerate() {
                        lines.insert(pos + 1 + offset, item);
                    }
                }
            }

            // 写入多条反向代理规则（支持 CORS 与 WebSocket）
            let valid_rules: Vec<&NginxProxyRule> = proxy_rules
                .iter()
                .filter(|r| !r.prefix.trim().is_empty() && !r.target.trim().is_empty())
                .collect();
            if !valid_rules.is_empty() {
                let mut block = vec!["        # MOKA_PROXY_BEGIN".to_string()];
                for r in valid_rules {
                    block.push(format!("        location {} {{", r.prefix.trim()));
                    block.push(format!("            proxy_pass {};", r.target.trim()));
                    block.push("            proxy_set_header Host $host;".to_string());
                    block.push("            proxy_set_header X-Real-IP $remote_addr;".to_string());
                    block.push(
                        "            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;"
                            .to_string(),
                    );
                    if r.websocket {
                        block.push("            proxy_http_version 1.1;".to_string());
                        block.push("            proxy_set_header Upgrade $http_upgrade;".to_string());
                        block.push("            proxy_set_header Connection \"upgrade\";".to_string());
                    }
                    if r.cors {
                        block.push(
                            "            add_header Access-Control-Allow-Origin * always;"
                                .to_string(),
                        );
                        block.push(
                            "            add_header Access-Control-Allow-Methods 'GET, POST, PUT, DELETE, OPTIONS' always;"
                                .to_string(),
                        );
                        block.push(
                            "            add_header Access-Control-Allow-Headers '*' always;"
                                .to_string(),
                        );
                    }
                    block.push("        }".to_string());
                }
                block.push("        # MOKA_PROXY_END".to_string());

                if let Some(idx) = lines.iter().rposition(|l| l.trim() == "}") {
                    let server_close = lines[..idx]
                        .iter()
                        .rposition(|l| l.trim() == "}")
                        .unwrap_or(idx);
                    for (offset, item) in block.into_iter().enumerate() {
                        lines.insert(server_close + offset, item);
                    }
                }
            }
        }
        VisualConfig::Nacos {
            port,
            context_path,
            auth_enabled,
            auth_secret_key,
            use_mysql,
            mysql_url,
            mysql_user,
            mysql_password,
        } => {
            upsert_prop(&mut lines, "server.port", port.trim());
            upsert_prop(&mut lines, "server.servlet.contextPath", context_path.trim());
            upsert_prop(
                &mut lines,
                "nacos.core.auth.enabled",
                if *auth_enabled { "true" } else { "false" },
            );
            if !auth_secret_key.trim().is_empty() {
                upsert_prop(
                    &mut lines,
                    "nacos.core.auth.plugin.nacos.token.secret.key",
                    auth_secret_key.trim(),
                );
            }
            if *use_mysql {
                upsert_prop(&mut lines, "spring.sql.init.platform", "mysql");
                upsert_prop(&mut lines, "db.num", "1");
                upsert_prop(&mut lines, "db.url.0", mysql_url.trim());
                upsert_prop(&mut lines, "db.user.0", mysql_user.trim());
                upsert_prop(&mut lines, "db.password.0", mysql_password.trim());
            } else {
                comment_prop(&mut lines, "spring.sql.init.platform");
                comment_prop(&mut lines, "spring.datasource.platform");
                comment_prop(&mut lines, "db.num");
            }
        }
        VisualConfig::Postgres {
            port,
            listen_addresses,
            max_connections,
            shared_buffers,
            work_mem,
            timezone,
            log_statement,
        } => {
            upsert_pg_kv(&mut lines, "port", port.trim(), false);
            upsert_pg_kv(&mut lines, "listen_addresses", listen_addresses.trim(), true);
            upsert_pg_kv(&mut lines, "max_connections", max_connections.trim(), false);
            upsert_pg_kv(&mut lines, "shared_buffers", shared_buffers.trim(), false);
            upsert_pg_kv(&mut lines, "work_mem", work_mem.trim(), false);
            upsert_pg_kv(&mut lines, "timezone", timezone.trim(), true);
            upsert_pg_kv(&mut lines, "log_statement", log_statement.trim(), true);
        }
        VisualConfig::Tomcat {
            port,
            shutdown_port,
            uri_encoding,
            max_threads,
            connection_timeout,
        } => {
            let mut xml = content.to_string();
            xml = update_xml_connector_attr(&xml, "port", port.trim());
            xml = update_xml_connector_attr(&xml, "URIEncoding", uri_encoding.trim());
            xml = update_xml_connector_attr(&xml, "maxThreads", max_threads.trim());
            xml = update_xml_connector_attr(&xml, "connectionTimeout", connection_timeout.trim());
            xml = update_xml_server_port(&xml, shutdown_port.trim());
            return xml;
        }
        VisualConfig::MongoDb {
            port,
            bind_ip,
            db_path,
            auth_enabled,
        } => {
            for line in &mut lines {
                let t = line.trim();
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                if t.starts_with("port:") {
                    *line = format!("{indent}port: {}", port.trim());
                } else if t.starts_with("bindIp:") {
                    *line = format!("{indent}bindIp: {}", bind_ip.trim());
                } else if t.starts_with("dbPath:") && !db_path.trim().is_empty() {
                    let norm = db_path.trim().replace('\\', "/");
                    *line = format!("{indent}dbPath: \"{norm}\"");
                } else if t.starts_with("authorization:") {
                    *line = format!(
                        "{indent}authorization: {}",
                        if *auth_enabled { "enabled" } else { "disabled" }
                    );
                }
            }
        }
        VisualConfig::Mailpit {
            port,
            smtp_port,
            bind_host,
            max_messages,
            accept_any_auth,
        } => {
            let host = if bind_host.trim().is_empty() {
                "127.0.0.1"
            } else {
                bind_host.trim()
            };
            upsert_prop(
                &mut lines,
                "MP_UI_BIND_ADDR",
                &format!("{host}:{}", port.trim()),
            );
            upsert_prop(
                &mut lines,
                "MP_SMTP_BIND_ADDR",
                &format!("{host}:{}", smtp_port.trim()),
            );
            upsert_prop(&mut lines, "MP_MAX_MESSAGES", max_messages.trim());
            upsert_prop(
                &mut lines,
                "MP_SMTP_AUTH_ACCEPT_ANY",
                if *accept_any_auth { "1" } else { "0" },
            );
            upsert_prop(
                &mut lines,
                "MP_SMTP_AUTH_ALLOW_INSECURE",
                if *accept_any_auth { "1" } else { "0" },
            );
        }
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn upsert_prop(lines: &mut Vec<String>, key: &str, value: &str) {
    let prefix = format!("{key}=");
    let commented_prefix_1 = format!("#{key}=");
    let commented_prefix_2 = format!("# {key}=");
    let mut active_idx = None;
    let mut commented_idx = None;

    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with(&prefix) {
            active_idx = Some(i);
            break;
        }
        if commented_idx.is_none()
            && (t.starts_with(&commented_prefix_1) || t.starts_with(&commented_prefix_2))
        {
            commented_idx = Some(i);
        }
    }

    let new_line = format!("{key}={value}");
    if let Some(i) = active_idx {
        lines[i] = new_line;
    } else if let Some(i) = commented_idx {
        lines[i] = new_line;
    } else {
        lines.push(new_line);
    }
}

fn comment_prop(lines: &mut [String], key: &str) {
    let prefix = format!("{key}=");
    for line in lines.iter_mut() {
        if line.trim().starts_with(&prefix) {
            *line = format!("# {}", line.trim());
        }
    }
}

fn upsert_pg_kv(lines: &mut Vec<String>, key: &str, value: &str, quote: bool) {
    let formatted_val = if quote {
        format!("'{value}'")
    } else {
        value.to_string()
    };
    let new_line = format!("{key} = {formatted_val}");
    let mut active_idx = None;
    let mut commented_idx = None;

    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix(key) {
            let r = rest.trim_start();
            if r.starts_with('=') {
                active_idx = Some(i);
                break;
            }
        }
        if commented_idx.is_none() {
            if let Some(uncommented) = t.strip_prefix('#').map(|s| s.trim_start()) {
                if let Some(rest) = uncommented.strip_prefix(key) {
                    if rest.trim_start().starts_with('=') {
                        commented_idx = Some(i);
                    }
                }
            }
        }
    }

    if let Some(i) = active_idx {
        lines[i] = new_line;
    } else if let Some(i) = commented_idx {
        lines[i] = new_line;
    } else {
        lines.push(new_line);
    }
}

fn find_http_connector_range(xml: &str) -> Option<(usize, usize)> {
    let mut search_from = 0;
    while let Some(rel_start) = xml[search_from..].find("<Connector") {
        let start = search_from + rel_start;
        let Some(rel_end) = xml[start..].find("/>") else {
            break;
        };
        let end = start + rel_end + 2;
        let slice = &xml[start..end];
        if slice.contains("HTTP/1.1") || slice.contains("Http11") {
            return Some((start, end));
        }
        search_from = end;
    }
    None
}

fn find_http_connector_slice(xml: &str) -> Option<&str> {
    let (s, e) = find_http_connector_range(xml)?;
    Some(&xml[s..e])
}

fn find_server_tag_slice(xml: &str) -> Option<&str> {
    let start = xml.find("<Server")?;
    let end = xml[start..].find('>')? + start + 1;
    Some(&xml[start..end])
}

fn extract_xml_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let idx = tag.find(&needle)? + needle.len();
    let end = tag[idx..].find('"')? + idx;
    Some(tag[idx..end].to_string())
}

fn update_xml_connector_attr(xml: &str, attr: &str, val: &str) -> String {
    let Some((start, end)) = find_http_connector_range(xml) else {
        return xml.to_string();
    };
    let slice = &xml[start..end];
    let needle = format!("{attr}=\"");
    let replaced = if let Some(rel) = slice.find(&needle) {
        let val_start = rel + needle.len();
        if let Some(val_len) = slice[val_start..].find('"') {
            format!(
                "{}{}{}",
                &slice[..val_start],
                val,
                &slice[val_start + val_len..]
            )
        } else {
            slice.to_string()
        }
    } else if let Some(close) = slice.rfind("/>") {
        format!("{} {attr}=\"{val}\" />", slice[..close].trim_end())
    } else {
        slice.to_string()
    };
    format!("{}{}{}", &xml[..start], replaced, &xml[end..])
}

fn update_xml_server_port(xml: &str, port: &str) -> String {
    let Some(start) = xml.find("<Server") else {
        return xml.to_string();
    };
    let Some(rel_end) = xml[start..].find('>') else {
        return xml.to_string();
    };
    let end = start + rel_end + 1;
    let slice = &xml[start..end];
    let needle = "port=\"";
    if let Some(rel) = slice.find(needle) {
        let val_start = rel + needle.len();
        if let Some(val_len) = slice[val_start..].find('"') {
            let replaced = format!(
                "{}{}{}",
                &slice[..val_start],
                port,
                &slice[val_start + val_len..]
            );
            return format!("{}{}{}", &xml[..start], replaced, &xml[end..]);
        }
    }
    xml.to_string()
}

/// 生成带阿里云镜像源的 Maven settings.xml
pub fn generate_aliyun_maven_settings(maven_conf_dir: &Path, local_repo: &Path) -> Result<PathBuf> {
    fs::create_dir_all(maven_conf_dir)?;
    fs::create_dir_all(local_repo)?;
    let path = maven_conf_dir.join("settings.xml");
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<settings xmlns="http://maven.apache.org/SETTINGS/1.2.0"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
          xsi:schemaLocation="http://maven.apache.org/SETTINGS/1.2.0 https://maven.apache.org/xsd/settings-1.2.0.xsd">
  <localRepository>{}</localRepository>
  <mirrors>
    <mirror>
      <id>aliyunmaven</id>
      <mirrorOf>*</mirrorOf>
      <name>阿里云公共仓库</name>
      <url>https://maven.aliyun.com/repository/public</url>
    </mirror>
  </mirrors>
</settings>
"#,
        to_slash(local_repo)
    );
    fs::write(&path, content)?;
    Ok(path)
}

/// 确保本地开发根证书（Moka Local Dev CA）存在并返回 `(ca_cert_path, ca_key_path)`
pub fn ensure_local_root_ca(ca_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(ca_dir)?;
    let cert_path = ca_dir.join("moka-root-ca.crt");
    let key_path = ca_dir.join("moka-root-ca.key");
    if cert_path.exists() && key_path.exists() {
        return Ok((cert_path, key_path));
    }

    let mut params = rcgen::CertificateParams::default();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    let mut dn = rcgen::DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, "Moka Local Dev CA");
    dn.push(rcgen::DnType::OrganizationName, "Moka Dev");
    params.distinguished_name = dn;

    let ca_key = rcgen::KeyPair::generate()?;
    let ca_cert = params.self_signed(&ca_key)?;
    fs::write(&cert_path, ca_cert.pem())?;
    fs::write(&key_path, ca_key.serialize_pem())?;
    Ok((cert_path, key_path))
}

/// 一键生成由 Moka Local Dev CA 签发的本地 HTTPS 站点证书（含 localhost、127.0.0.1 与目标域名）
pub fn generate_local_ssl_cert(ssl_dir: &Path, server_name: &str) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(ssl_dir)?;
    // CA 统一存放在 Moka-Runtime/conf/ca（从 ssl_dir 向上定位 conf 目录，否则退回到 ssl_dir/ca）
    let ca_dir = ssl_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|conf_root| conf_root.join("ca"))
        .unwrap_or_else(|| ssl_dir.join("ca"));
    let (ca_cert_path, ca_key_path) = ensure_local_root_ca(&ca_dir)?;

    let ca_cert_pem = fs::read_to_string(&ca_cert_path)?;
    let ca_key_pem = fs::read_to_string(&ca_key_path)?;
    let ca_key = rcgen::KeyPair::from_pem(&ca_key_pem)?;
    let ca_params = rcgen::CertificateParams::from_ca_cert_pem(&ca_cert_pem)?;
    let ca_cert = ca_params.self_signed(&ca_key)?;

    let mut sans = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    let clean_name = server_name.split_whitespace().next().unwrap_or("localhost");
    if !clean_name.is_empty() && !sans.iter().any(|s| s.eq_ignore_ascii_case(clean_name)) {
        sans.push(clean_name.to_string());
    }

    let mut leaf_params = rcgen::CertificateParams::new(sans)?;
    leaf_params.is_ca = rcgen::IsCa::NoCa;
    leaf_params.key_usages = vec![
        rcgen::KeyUsagePurpose::DigitalSignature,
        rcgen::KeyUsagePurpose::KeyEncipherment,
    ];
    leaf_params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let mut leaf_dn = rcgen::DistinguishedName::new();
    leaf_dn.push(rcgen::DnType::CommonName, clean_name);
    leaf_params.distinguished_name = leaf_dn;

    let leaf_key = rcgen::KeyPair::generate()?;
    let leaf_cert = leaf_params.signed_by(&leaf_key, &ca_cert, &ca_key)?;

    let file_stem: String = clean_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = if file_stem.is_empty() {
        "localhost".to_string()
    } else {
        file_stem
    };
    let cert_path = ssl_dir.join(format!("{stem}.crt"));
    let key_path = ssl_dir.join(format!("{stem}.key"));
    // 写入完整证书链（站点证书 + 本地根证书）
    let fullchain = format!("{}\n{}", leaf_cert.pem(), ca_cert_pem);
    fs::write(&cert_path, fullchain)?;
    fs::write(&key_path, leaf_key.serialize_pem())?;
    Ok((cert_path, key_path))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteProxyRule {
    pub prefix: String,
    pub target: String,
    #[serde(default)]
    pub cors: bool,
    #[serde(default)]
    pub websocket: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub id: String,
    pub domain: String,
    #[serde(default = "default_site_http_port")]
    pub port: u16,
    #[serde(default)]
    pub root_dir: String,
    #[serde(default = "default_site_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub ssl_enabled: bool,
    #[serde(default = "default_site_ssl_port")]
    pub ssl_port: u16,
    #[serde(default)]
    pub ssl_cert: String,
    #[serde(default)]
    pub ssl_key: String,
    #[serde(default)]
    pub proxy_rules: Vec<SiteProxyRule>,
}

fn default_site_http_port() -> u16 {
    80
}
fn default_site_ssl_port() -> u16 {
    443
}
fn default_site_enabled() -> bool {
    true
}

/// 生成单个站点的 Nginx vhost 配置文本
pub fn render_site_vhost_conf(site: &SiteConfig, fallback_root: &Path) -> String {
    let root = if site.root_dir.trim().is_empty() {
        to_slash(fallback_root)
    } else {
        site.root_dir.trim().replace('\\', "/")
    };
    let mut lines = vec![
        format!("# Moka 托管站点: {}", site.domain),
        "server {".to_string(),
        format!("    listen       {};", site.port),
        format!("    server_name  {};", site.domain.trim()),
    ];
    if site.ssl_enabled && !site.ssl_cert.trim().is_empty() && !site.ssl_key.trim().is_empty() {
        let cert_norm = site.ssl_cert.trim().replace('\\', "/");
        let key_norm = site.ssl_key.trim().replace('\\', "/");
        lines.push(format!("    listen       {} ssl;", site.ssl_port));
        lines.push(format!("    ssl_certificate     \"{cert_norm}\";"));
        lines.push(format!("    ssl_certificate_key \"{key_norm}\";"));
        lines.push("    ssl_protocols       TLSv1.2 TLSv1.3;".to_string());
    }
    lines.push("    location / {".to_string());
    lines.push(format!("        root   \"{root}\";"));
    lines.push("        index  index.html index.htm;".to_string());
    lines.push("    }".to_string());

    for r in &site.proxy_rules {
        if r.prefix.trim().is_empty() || r.target.trim().is_empty() {
            continue;
        }
        lines.push(format!("    location {} {{", r.prefix.trim()));
        lines.push(format!("        proxy_pass {};", r.target.trim()));
        lines.push("        proxy_set_header Host $host;".to_string());
        lines.push("        proxy_set_header X-Real-IP $remote_addr;".to_string());
        lines.push(
            "        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;".to_string(),
        );
        if r.websocket {
            lines.push("        proxy_http_version 1.1;".to_string());
            lines.push("        proxy_set_header Upgrade $http_upgrade;".to_string());
            lines.push("        proxy_set_header Connection \"upgrade\";".to_string());
        }
        if r.cors {
            lines.push("        add_header Access-Control-Allow-Origin * always;".to_string());
            lines.push(
                "        add_header Access-Control-Allow-Methods 'GET, POST, PUT, DELETE, OPTIONS' always;"
                    .to_string(),
            );
            lines.push("        add_header Access-Control-Allow-Headers '*' always;".to_string());
        }
        lines.push("    }".to_string());
    }

    lines.push("}".to_string());
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// 将所有启用站点写入 `vhosts/*.conf`，并确保主 `nginx.conf` 的 `http { ... }` 包含 `include` 指令
pub fn sync_nginx_vhosts(
    nginx_conf_path: &Path,
    vhosts_dir: &Path,
    sites_data_root: &Path,
    sites: &[SiteConfig],
) -> Result<()> {
    fs::create_dir_all(vhosts_dir)?;
    // 清理旧的 Moka vhost 文件
    if let Ok(entries) = fs::read_dir(vhosts_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("conf") {
                let _ = fs::remove_file(p);
            }
        }
    }

    for site in sites.iter().filter(|s| s.enabled && !s.domain.trim().is_empty()) {
        let safe_name: String = site
            .domain
            .trim()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let fallback_root = sites_data_root.join(&safe_name);
        if site.root_dir.trim().is_empty() {
            fs::create_dir_all(&fallback_root)?;
            let idx = fallback_root.join("index.html");
            if !idx.exists() {
                let _ = fs::write(
                    idx,
                    format!(
                        "<!doctype html><html><head><meta charset='utf-8'><title>{}</title></head>\
                         <body style='font-family:sans-serif;padding:40px'><h1>{} 运行中</h1>\
                         <p>由 Moka 站点管理自动生成，你可以在站点编辑面板自定义根目录与反向代理规则。</p></body></html>",
                        site.domain, site.domain
                    ),
                );
            }
        }
        let conf_txt = render_site_vhost_conf(site, &fallback_root);
        fs::write(vhosts_dir.join(format!("{safe_name}.conf")), conf_txt)?;
    }

    // 确保 nginx.conf 包含 `include "<vhosts_dir>/*.conf";`
    if nginx_conf_path.exists() {
        let raw = fs::read_to_string(nginx_conf_path)?;
        let include_line = format!("    include       \"{}/*.conf\";", to_slash(vhosts_dir));
        if !raw.contains(&include_line) {
            let mut lines: Vec<String> = raw
                .lines()
                .filter(|l| !l.contains("/vhosts/*.conf"))
                .map(|l| l.to_string())
                .collect();
            if let Some(last_brace) = lines.iter().rposition(|l| l.trim() == "}") {
                lines.insert(last_brace, include_line);
                let mut updated = lines.join("\n");
                updated.push('\n');
                fs::write(nginx_conf_path, updated)?;
            }
        }
    }

    Ok(())
}

/// 开发包管理器镜像源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevMirrorKind {
    Npm,
    Pip,
    Go,
    Maven,
}

impl DevMirrorKind {
    pub const ALL: [DevMirrorKind; 4] = [
        DevMirrorKind::Npm,
        DevMirrorKind::Pip,
        DevMirrorKind::Go,
        DevMirrorKind::Maven,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Npm => "npm / Node.js",
            Self::Pip => "pip / Python",
            Self::Go => "Go GOPROXY",
            Self::Maven => "Maven / Java",
        }
    }

    pub fn china_desc(self) -> &'static str {
        match self {
            Self::Npm => "淘宝 npmmirror (registry.npmmirror.com)",
            Self::Pip => "清华 TUNA (pypi.tuna.tsinghua.edu.cn)",
            Self::Go => "七牛云 Goproxy (goproxy.cn)",
            Self::Maven => "阿里云 Maven (maven.aliyun.com)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DevMirrorState {
    pub kind: DevMirrorKind,
    pub is_china: bool,
    pub current_url: String,
    pub config_path: PathBuf,
}

fn user_home_dir() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn user_appdata_dir() -> PathBuf {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| user_home_dir().join("AppData").join("Roaming"))
}

pub fn dev_mirror_config_path(kind: DevMirrorKind) -> PathBuf {
    match kind {
        DevMirrorKind::Npm => user_home_dir().join(".npmrc"),
        DevMirrorKind::Pip => user_appdata_dir().join("pip").join("pip.ini"),
        DevMirrorKind::Go => user_appdata_dir().join("go").join("env"),
        DevMirrorKind::Maven => user_home_dir().join(".m2").join("settings.xml"),
    }
}

fn upsert_kv_line(text: &str, key: &str, sep: &str, new_val: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut found = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            if k.trim().eq_ignore_ascii_case(key) {
                *line = format!("{key}{sep}{new_val}");
                found = true;
            }
        }
    }
    if !found {
        lines.push(format!("{key}{sep}{new_val}"));
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn is_china_mirror_url(url: &str) -> bool {
    let u = url.to_lowercase();
    u.contains("npmmirror")
        || u.contains("taobao")
        || u.contains("tsinghua")
        || u.contains("aliyun")
        || u.contains("goproxy.cn")
        || u.contains("goproxy.io")
        || u.contains("huaweicloud")
        || u.contains("ustc.edu.cn")
        || u.contains("tencent")
}

/// 检测所有 4 个主流包管理器的当前镜像源状态
pub fn inspect_dev_mirrors() -> Vec<DevMirrorState> {
    DevMirrorKind::ALL
        .into_iter()
        .map(|kind| {
            let path = dev_mirror_config_path(kind);
            let content = fs::read_to_string(&path).unwrap_or_default();
            let (is_china, current_url) = match kind {
                DevMirrorKind::Npm => {
                    let url = content
                        .lines()
                        .filter_map(|l| {
                            let t = l.trim();
                            if t.starts_with('#') || t.starts_with(';') {
                                return None;
                            }
                            let (k, v) = t.split_once('=')?;
                            if k.trim().eq_ignore_ascii_case("registry") {
                                Some(v.trim().to_string())
                            } else {
                                None
                            }
                        })
                        .next_back()
                        .unwrap_or_else(|| "https://registry.npmjs.org/".to_string());
                    (is_china_mirror_url(&url), url)
                }
                DevMirrorKind::Pip => {
                    let url = content
                        .lines()
                        .filter_map(|l| {
                            let t = l.trim();
                            if t.starts_with('#') || t.starts_with(';') {
                                return None;
                            }
                            let (k, v) = t.split_once('=')?;
                            if k.trim().eq_ignore_ascii_case("index-url") {
                                Some(v.trim().to_string())
                            } else {
                                None
                            }
                        })
                        .next_back()
                        .unwrap_or_else(|| "https://pypi.org/simple".to_string());
                    (is_china_mirror_url(&url), url)
                }
                DevMirrorKind::Go => {
                    let url = content
                        .lines()
                        .filter_map(|l| {
                            let (k, v) = l.trim().split_once('=')?;
                            if k.trim().eq_ignore_ascii_case("GOPROXY") {
                                Some(v.trim().to_string())
                            } else {
                                None
                            }
                        })
                        .next_back()
                        .unwrap_or_else(|| "https://proxy.golang.org,direct".to_string());
                    (is_china_mirror_url(&url), url)
                }
                DevMirrorKind::Maven => {
                    let china = is_china_mirror_url(&content);
                    let url = if china {
                        "https://maven.aliyun.com/repository/public".to_string()
                    } else {
                        "https://repo.maven.apache.org/maven2".to_string()
                    };
                    (china, url)
                }
            };
            DevMirrorState {
                kind,
                is_china,
                current_url,
                config_path: path,
            }
        })
        .collect()
}

/// 切换单个包管理器的镜像源（true = 国内高速镜像，false = 恢复官方源）
pub fn set_dev_mirror(kind: DevMirrorKind, use_china: bool) -> Result<PathBuf> {
    let path = dev_mirror_config_path(kind);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match kind {
        DevMirrorKind::Npm => {
            let existing = fs::read_to_string(&path).unwrap_or_default();
            let target = if use_china {
                "https://registry.npmmirror.com/"
            } else {
                "https://registry.npmjs.org/"
            };
            let updated = upsert_kv_line(&existing, "registry", "=", target);
            fs::write(&path, updated)?;
        }
        DevMirrorKind::Pip => {
            let content = if use_china {
                "[global]\nindex-url = https://pypi.tuna.tsinghua.edu.cn/simple\ntrusted-host = pypi.tuna.tsinghua.edu.cn\n"
            } else {
                "[global]\nindex-url = https://pypi.org/simple\n"
            };
            fs::write(&path, content)?;
        }
        DevMirrorKind::Go => {
            let existing = fs::read_to_string(&path).unwrap_or_default();
            let target = if use_china {
                "https://goproxy.cn,direct"
            } else {
                "https://proxy.golang.org,direct"
            };
            let updated = upsert_kv_line(&existing, "GOPROXY", "=", target);
            fs::write(&path, updated)?;
        }
        DevMirrorKind::Maven => {
            let bak = path.with_extension("xml.moka.bak");
            if use_china {
                if path.exists() && !bak.exists() {
                    if let Ok(cur) = fs::read_to_string(&path) {
                        if !cur.contains("moka-aliyun-mirror") {
                            let _ = fs::copy(&path, &bak);
                        }
                    }
                }
                let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- Generated by Moka Dev Mirror Accelerator (moka-aliyun-mirror) -->
<settings xmlns="http://maven.apache.org/SETTINGS/1.0.0"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
          xsi:schemaLocation="http://maven.apache.org/SETTINGS/1.0.0 https://maven.apache.org/xsd/settings-1.0.0.xsd">
  <mirrors>
    <mirror>
      <id>aliyunmaven</id>
      <mirrorOf>*</mirrorOf>
      <name>阿里云公共仓库</name>
      <url>https://maven.aliyun.com/repository/public</url>
    </mirror>
  </mirrors>
</settings>
"#;
                fs::write(&path, xml)?;
            } else if bak.exists() {
                fs::copy(&bak, &path)?;
                let _ = fs::remove_file(&bak);
            } else {
                let official_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<settings xmlns="http://maven.apache.org/SETTINGS/1.0.0"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
          xsi:schemaLocation="http://maven.apache.org/SETTINGS/1.0.0 https://maven.apache.org/xsd/settings-1.0.0.xsd">
</settings>
"#;
                fs::write(&path, official_xml)?;
            }
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_service_visual_configs_roundtrip() {
        let app = Path::new("C:/Moka/apps/test");
        let data = Path::new("C:/Moka/data/test");
        let cases: [(&str, u16); 8] = [
            ("mysql", 3308),
            ("redis", 6379),
            ("nginx", 80),
            ("nacos", 8848),
            ("postgres", 5432),
            ("tomcat", 8080),
            ("mongodb", 27017),
            ("mailpit", 8025),
        ];

        for (kind, port) in cases {
            let tpl = default_service_config_content(Some(kind), app, data, port)
                .unwrap_or_else(|| panic!("missing default template for {kind}"));
            let parsed = parse_visual_config(Some(kind), &tpl, port)
                .unwrap_or_else(|| panic!("parse_visual_config failed for {kind}"));
            let applied = apply_visual_config(&tpl, &parsed);
            assert!(!applied.trim().is_empty(), "{kind} 写回结果不应为空");
            let reparsed = parse_visual_config(Some(kind), &applied, port)
                .unwrap_or_else(|| panic!("re-parse failed for {kind}"));
            match reparsed {
                VisualConfig::Mysql { port: p, .. }
                | VisualConfig::Redis { port: p, .. }
                | VisualConfig::Nginx { port: p, .. }
                | VisualConfig::Nacos { port: p, .. }
                | VisualConfig::Postgres { port: p, .. }
                | VisualConfig::Tomcat { port: p, .. }
                | VisualConfig::MongoDb { port: p, .. }
                | VisualConfig::Mailpit { port: p, .. } => {
                    assert_eq!(p, port.to_string(), "{kind} 端口往返不一致");
                }
            }
        }
    }

    #[test]
    fn read_service_port_returns_custom_port_from_disk() {
        let base = std::env::temp_dir().join(format!("moka-port-test-{}", std::process::id()));
        let app = base.join("apps/test");
        let conf = base.join("conf/test");
        let data = base.join("data/test");
        for kind in [
            "mysql", "redis", "nginx", "nacos", "postgres", "tomcat", "mongodb", "mailpit",
        ] {
            let tpl = default_service_config_content(Some(kind), &app, &data, 3310)
                .unwrap_or_else(|| panic!("missing default template for {kind}"));
            let path = service_config_path(Some(kind), &app, &conf, &data).unwrap();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &tpl).unwrap();
            assert_eq!(
                read_service_port(Some(kind), &app, &conf, &data, 80),
                Some(3310),
                "{kind} 应从配置文件读出自定义端口"
            );
        }
        assert_eq!(read_service_port(None, &app, &conf, &data, 80), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn nginx_multi_proxy_and_ssl_roundtrip() {
        let app = Path::new("C:/Moka/apps/nginx-1.26");
        let data = Path::new("C:/Moka/data/nginx-1.26");
        let tpl = default_service_config_content(Some("nginx"), app, data, 80).unwrap();
        let mut cfg = parse_visual_config(Some("nginx"), &tpl, 80).unwrap();
        if let VisualConfig::Nginx {
            ref mut proxy_rules,
            ref mut ssl_enabled,
            ref mut ssl_port,
            ref mut ssl_cert,
            ref mut ssl_key,
            ..
        } = cfg
        {
            proxy_rules.push(NginxProxyRule {
                prefix: "/api/".into(),
                target: "http://127.0.0.1:8080".into(),
                cors: true,
                websocket: false,
            });
            proxy_rules.push(NginxProxyRule {
                prefix: "/ws/".into(),
                target: "http://127.0.0.1:8081".into(),
                cors: false,
                websocket: true,
            });
            *ssl_enabled = true;
            *ssl_port = "8443".into();
            *ssl_cert = "C:/Moka/ssl/localhost.crt".into();
            *ssl_key = "C:/Moka/ssl/localhost.key".into();
        }
        let written = apply_visual_config(&tpl, &cfg);
        assert!(written.contains("listen       8443 ssl;"));
        assert!(written.contains("location /api/"));
        assert!(written.contains("location /ws/"));
        assert!(written.contains("Upgrade $http_upgrade"));

        let reparsed = parse_visual_config(Some("nginx"), &written, 80).unwrap();
        if let VisualConfig::Nginx {
            proxy_rules,
            ssl_enabled,
            ssl_port,
            ..
        } = reparsed
        {
            assert!(ssl_enabled);
            assert_eq!(ssl_port, "8443");
            assert_eq!(proxy_rules.len(), 2);
            assert!(proxy_rules[0].cors);
            assert!(proxy_rules[1].websocket);
        } else {
            panic!("expected Nginx visual config");
        }
    }

    #[test]
    fn mongodb_yaml_preserves_indentation() {
        let tpl = default_service_config_content(
            Some("mongodb"),
            Path::new("C:/a"),
            Path::new("C:/d"),
            27017,
        )
        .unwrap();
        assert!(tpl.contains("storage:\n  dbPath:"));
        assert!(tpl.contains("net:\n  bindIp: 127.0.0.1\n  port: 27017"));
        assert!(tpl.contains("security:\n  authorization: disabled"));
    }

    #[test]
    fn local_root_ca_and_leaf_cert_generation() {
        let tmp = std::env::temp_dir().join(format!(
            "moka_ca_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let (ca_crt, ca_key) = ensure_local_root_ca(&tmp).expect("ensure_local_root_ca");
        assert!(ca_crt.exists());
        assert!(ca_key.exists());

        let (leaf_crt, leaf_key) =
            generate_local_ssl_cert(&tmp, "api.local").expect("generate_local_ssl_cert");
        assert!(leaf_crt.exists());
        assert!(leaf_key.exists());

        let fullchain = std::fs::read_to_string(&leaf_crt).unwrap();
        assert_eq!(
            fullchain.matches("BEGIN CERTIFICATE").count(),
            2,
            "leaf cert should include CA certificate chain"
        );
        let key_pem = std::fs::read_to_string(&leaf_key).unwrap();
        assert!(key_pem.contains("BEGIN PRIVATE KEY"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn upsert_kv_line_preserves_other_entries() {
        let npmrc = "//registry.npmjs.org/:_authToken=secret\nregistry=https://registry.npmjs.org/\n";
        let china = upsert_kv_line(npmrc, "registry", "=", "https://registry.npmmirror.com/");
        assert!(china.contains("_authToken=secret"));
        assert!(china.contains("registry=https://registry.npmmirror.com/"));
        assert!(!china.contains("registry=https://registry.npmjs.org/"));

        let go_env = "GOSUMDB=sum.golang.org\n";
        let go_china = upsert_kv_line(go_env, "GOPROXY", "=", "https://goproxy.cn,direct");
        assert!(go_china.contains("GOSUMDB=sum.golang.org"));
        assert!(go_china.contains("GOPROXY=https://goproxy.cn,direct"));
    }

    #[test]
    fn sync_port_preserves_nginx_listen_modifiers() {
        let conf = "server {\n    listen 127.0.0.1:80 default_server;\n    listen 8081;\n}\n";
        let updated = sync_port_in_config(Some("nginx"), conf, 8080);
        assert!(updated.contains("    listen       127.0.0.1:8080 default_server;"));
        assert!(
            updated.contains("    listen 8081;"),
            "secondary server listen should not be overwritten"
        );
    }
}
