#import "template.typ": *;
#import "style.typ": *;

#import "fp.typ": *;
// #import "object.typ": *;
#import "class.typ": *;
#import "iterator.typ": *;

#import "mth.typ";

#let this-doc = state("this-doc", document);

/// use in bundle main.typ
#let bundle_document(name) = {
    import name + ".typ" as p;
    let title = dictionary(p).at("title", default: none);
    let tag = dictionary(p).at("tag", default: none);
    name += ".html";
    this-path.update(x => name);
    [#document(name, title: title, context[#this-doc.update(here());#p;]) #tag]
}
/// use in bundle main.typ
#let bundle_asset(name) = {
    asset(name, read(name, encoding: none))
}

#let custom_element(register, ele) = {
    [#metadata(register) <custom-element>]
    ele
}
