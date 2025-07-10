const menuIcon = document.getElementById("menu-icon");
menuIcon.addEventListener("click", () => {
   menuIcon.hidden = !menuIcon.hidden;
   const menu = document.getElementById("side-menu");
   menu.classList.toggle("-translate-x-full");
   menu.classList.toggle("translate-x-0");
});

