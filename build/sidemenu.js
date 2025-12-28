const body = document.body;
const menu = document.getElementById('side-menu');
const menuIcon = document.getElementById('menu-icon');

menuIcon.addEventListener('click', () => {
   menu.classList.toggle('-translate-x-full');
   body.classList.toggle('menu-open');
}); 
