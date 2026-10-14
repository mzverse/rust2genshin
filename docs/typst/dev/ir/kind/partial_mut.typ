#import "/lib/lib.typ": *;
#show: template;

#let title = [PartialMut];
#let tag = <tag-partial-mut>

#set raw(lang: "rust");

`&mut T`的值部分，当`T`是ADT

表示对一个对象的引用

lower后等同于`T`，但不允许储存
