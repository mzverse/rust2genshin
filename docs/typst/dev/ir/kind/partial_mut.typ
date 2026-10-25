#import "/lib/lib.typ": *;
#show: template;

#let title = [PartialMut];
#let tag = <tag-partial-mut>

#set raw(lang: "rust");

可变视图：值就是被视图对象的值，但标记着「这是通过引用拿到的」

局部变量交出来的永远是视图，读值要过`Copy`节点

lower后等同于被视图的类型
