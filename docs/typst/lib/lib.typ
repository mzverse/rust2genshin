#import "template.typ": *;
#import "style.typ": *;

#import "fp.typ": *;
// #import "object.typ": *;
#import "class.typ": *;
#import "iterator.typ": *;

#import "mth.typ";

#let this-doc() = query(document.where().before(here())).last();

#let current-order = state("current-order");

/// use in bundle main.typ
#let bundle_document(name) = {
    import name + ".typ" as p;
    let dic = dictionary(p);
    let title = dic.at("title", default: none);
    let tag = dic.at("tag", default: none);
    name += ".html";
    this-path.update(x => name);
    context current-order.update(dic.at("order", default: float.inf));
    [#document(name, title: title, include p) #tag]
}
/// use in bundle main.typ
#let bundle_asset(name) = {
    asset(name, read(name, encoding: none))
}

#let custom_element(register, ele) = {
    [#metadata(register) <custom-element>]
    ele
}
