#import "/lib/lib.typ": *;
#show: template;

#let title = [kind];

#set raw(lang: "rust");

类型的中间表示，`IrKind::Native`表示原生类型

包含一些无法lower的类型，相关节点必须在优化时消除掉
