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
