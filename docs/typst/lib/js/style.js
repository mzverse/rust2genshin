document.addEventListener('mouseover', e => {
    const link = e.target.closest('a[href^="#"]');
    if(!link)
        return;
    document.querySelector(link.getAttribute('href')).classList.add('highlight');
});

document.addEventListener('mouseout', e => {
    const link = e.target.closest('a[href^="#"]');
    if(!link)
        return;
    document.querySelector(link.getAttribute('href')).classList.remove('highlight');
});
