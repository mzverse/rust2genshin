document.addEventListener('mouseover', e => {
    const link = e.target.closest('a[href^="#"]');
    let href;
    if(!link || (href = link.getAttribute('href')) === '#')
        return;
    document.querySelector(href).classList.add('highlight');
});

document.addEventListener('mouseout', e => {
    const link = e.target.closest('a[href^="#"]');
    let href;
    if(!link || (href = link.getAttribute('href')) === '#')
        return;
    document.querySelector(href).classList.remove('highlight');
});
