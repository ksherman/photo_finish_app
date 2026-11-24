<script lang="ts">
let nextClipId = 0;
</script>

<script setup lang="ts">
const clipId = `pf-brand-ap-clip-${++nextClipId}`;
</script>

<template>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="100 100 824 824"
    role="img"
    aria-label="PhotoFinish"
  >
    <defs>
      <clipPath :id="clipId">
        <circle cx="512" cy="512" r="280" />
      </clipPath>
    </defs>
    <rect x="100" y="100" width="824" height="824" rx="185" fill="#13233F" />
    <polygon points="360,100 470,100 552,380 442,380" fill="#F2B33D" />
    <polygon points="664,100 554,100 472,380 582,380" fill="#E2553A" />
    <circle cx="512" cy="540" r="270" fill="#F2B33D" stroke="#FFFFFF" stroke-width="24" />
    <g transform="translate(512 540) scale(0.771) translate(-512 -512)">
      <circle cx="512" cy="512" r="280" fill="#FFFFFF" />
      <g :clip-path="`url(#${clipId})`" stroke="#13233F" stroke-width="30" stroke-linecap="round">
        <line x1="633.2" y1="582" x2="633.2" y2="980" />
        <line x1="512" y1="652" x2="165.6" y2="852" />
        <line x1="390.8" y1="582" x2="44.4" y2="382" />
        <line x1="390.8" y1="442" x2="390.8" y2="42" />
        <line x1="512" y1="372" x2="858.4" y2="172" />
        <line x1="633.2" y1="442" x2="979.6" y2="642" />
      </g>
      <polygon
        points="633.2,582 512,652 390.8,582 390.8,442 512,372 633.2,442"
        fill="#13233F"
      />
    </g>
  </svg>
</template>
