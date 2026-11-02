#import "/lib/lib.typ": *;
#show: template;

#let title = [Never];
#let tag = <tag-never>;

#set raw(lang: "rust");

不可达类型，对应Rust中的`!`

仅用于类型编译，不会真正出现在节点图中
