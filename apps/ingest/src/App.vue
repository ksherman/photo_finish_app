<script setup lang="ts">
import { ref } from "vue";
import MainView from "@/views/MainView.vue";
import RenameView from "@/views/RenameView.vue";

const currentView = ref<"main" | "rename">("main");

function navigate(view: string) {
  currentView.value = view as "main" | "rename";
}
</script>

<template>
  <MainView v-if="currentView === 'main'" @navigate="navigate" />
  <RenameView v-else-if="currentView === 'rename'" @back="navigate('main')" />
</template>
