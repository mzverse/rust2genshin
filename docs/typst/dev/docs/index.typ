#import "/lib/lib.typ": *;
#let title = [文档];
#show: template;

使用Typst编写此文档，于`docs/typst`

= 安装typst

官网：https://typst.app

Windows: ```shell winget install --id Typst.Typst```

MacOS: ```shell brew install typst```

cargo：```shell cargo install --locked typst-cli```

= Build

编译此文档

```shell
rm -r target/debug/docs
cargo run -p rust2genshin-docs --bin compile
```

= Watch

编译此文档时，实时更新（增量编译）

启动watch并保持运行

```shell
cargo run -p rust2genshin-docs --bin watch
```

= 编辑器

== RustRover

目前已知仅可使用Scribe Pro插件（付费），请各显神通

=== 配置

安装插件时自动安装Tinymist（语言服务器）

设置 > 工具 > Typst >

- Compilation

    - Typst Root 设为 `docs/typst`

    - Extra Typst arguments 设为 `--features html,bundle`

- Language Server

    - Project resolution 可设为 Lock database (experimental) （增量解析）

== VS Code

安装插件Tinymist Typst并自动安装Tinymist

已于项目的`.vscode/settings.json`中正确配置：

```json
{
    "tinymist.rootPath": "${workspaceFolder}/docs/typst",
    "tinymist.projectResolution": "lockDatabase",
    "tinymist.exportTarget": "html"
}
```

= 编写文档

每个文档的开头固定为：

```typ
#import "/lib/lib.typ": *;
#let title = [请输入标题];
#show: template;
```
