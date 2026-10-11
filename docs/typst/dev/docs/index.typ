#import "/lib/lib.typ": *;
#let title = [文档];
#show: template;

使用Typst编写此文档，于`docs/typst`

= 安装typst

官网：https://typst.app

Windows: ```shell winget install --id Typst.Typst```

Homebrew（MacOS、Linux）: ```shell brew install typst```

Snap（Ubuntu）：```shell sudo snap install typst```

cargo：```shell cargo install --locked typst-cli```

= Build

编译此文档

```shell
rm -r target/debug/docs
cargo run -p rust2genshin-docs --bin compile
```

= Watch

编译并提供一个本地web服务

编辑此文档时，实时更新（增量编译），浏览器中也自动刷新

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

#set raw(lang: "typst");

每个文档的开头固定为：

```typst
#import "/lib/lib.typ": *;
#let title = [请输入标题];
#show: template;

#set raw(lang: "rust");
```

`#set raw(...)` 给页内的行内代码设默认语言 —— 不设的话 Typst 导出的 `<code>` 没有语言标注，高亮器只能自动猜。代码块自己的 ```` ```rust ```` / ```` ```shell ```` 优先级更高，不受影响。页面主体不是 Rust 时换成对应语言（本页这类讲 Typst / CSS 的就不要写 `rust`）。

注意几件容易踩的事：

- 加粗是 `*文字*`，不是 `**文字**`。写错会渲染成*空*的 `<strong>`，字不加粗，而且只有一条 `no text within stars` 警告
- 链接是 `#link("url")[文字]` 或裸 URL。`[文字](url)` 会被当字面量原样渲染
- 没有表格样式，别用 Markdown 的 `| a | b |`，用 `#list` 或 `#table(...)`

改样式之前先看#link("/dev/docs/layout.html")[排版]页，那三个坑不报错，只能靠知道。
