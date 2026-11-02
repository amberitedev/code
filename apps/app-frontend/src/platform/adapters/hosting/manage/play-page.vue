<script setup lang="ts">
import { injectServerPlay, ServersManagePlayPage } from '@modrinth/ui'

import { config } from '@/config'

const { play } = injectServerPlay()
</script>

<template>
	<ServersManagePlayPage :on-play-server="play" :site-url="config.accountSiteUrl" />
</template>
