import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import './style.css'

const theme = localStorage.getItem('mc-theme') ?? 'dark'
document.documentElement.dataset.theme = theme

createApp(App).use(createPinia()).mount('#app')
