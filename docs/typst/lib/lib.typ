#import "template.typ": *;
#import "style.typ": *;

#import "fp.typ": *;
// #import "object.typ": *;
#import "class.typ": *;
#import "iterator.typ": *;

#import "mth.typ";

/// use in bundle main.typ
#let bundle_document(name) = {
    import name + ".typ" as p;
    let dic = dictionary(p);
    let title = dic.at("title", default: none);
    let tag = dic.at("tag", default: none);
    state-this-doc-module.update(p);
    [#document(name + ".html", title: title, include p); #tag;]
}
/// use in bundle main.typ
#let bundle_asset(name) = {
    asset(name, read(name, encoding: none))
}

#let custom_element(register, ele) = {
    [#metadata(register) <custom-element>]
    ele
}
