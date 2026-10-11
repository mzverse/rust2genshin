#import "/lib/lib.typ": *;
#let title = [文档];
#show: template;

本项目的文档由Typst+css+js构建

Typst用于生成html

= Build

使用bundle和html特性

由Rust生成bundle的main.typ，其中包含所有文档和资源的路径

然后调用typst进行编译

= Watch

使用```shell typst watch```实时更新已有文件

Rust监听文件的增删并重建main.typ

= Base Path

== Rust

从环境变量`BASE_PATH`转移到```typst sys.inputs```

== Typst

存至`contents.base_path`

在生成`href`和`src`属性前手动添加

在`template`中将绝对路径的`link`处理为`html.a`
