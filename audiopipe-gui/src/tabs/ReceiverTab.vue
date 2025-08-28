<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { ref } from 'vue';

const targetAddress = ref("0.0.0.0");
function start_receiver() {
    
}

async function greet() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  //greetMsg.value = await invoke("greet", { name: name.value });
}
</script>

<template>
    <form @submit.prevent="start_receiver">
        <input type="text"></input>
        <input type="submit">
    </input>
    </form>
</template>