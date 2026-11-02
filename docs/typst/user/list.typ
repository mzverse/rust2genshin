#import "/lib/lib.typ": *;
#show: template;

#let title = [列表];

#set raw(lang: "rust");

可以理解为一个在栈上的`Vec`，也可*拓展*和*收缩*

在底层和*数组*相同，也能够相互转换
