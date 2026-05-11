<template>
	<div>
		<div class="relative w-full">
			<!-- DNS records section -->
			<div class="flex flex-col gap-2.5">
				<label for="user-domain" class="flex flex-col gap-2">
					<span class="text-lg font-semibold text-contrast">DNS records</span>
				</label>
				<div class="flex w-full flex-col items-center justify-start gap-2 sm:flex-row">
					<StyledInput
						id="user-domain"
						v-model="userDomain"
						wrapper-class="grow max-w-[400px]"
						:maxlength="64"
						:placeholder="exampleDomain"
					/>

					<ButtonStyled>
						<button
							class="!w-full sm:!w-auto"
							:disabled="userDomain == ''"
							@click="exportDnsRecords"
						>
							<UploadIcon />
							<span>Export</span>
						</button>
					</ButtonStyled>
				</div>

				<Table :columns="dnsColumns" :data="dnsRecords">
					<template #cell-type="{ row }">
						<TagItem
							v-if="row.type === 'SRV'"
							class="border !border-solid border-purple bg-highlight-purple !font-medium"
							:style="`--_color: var(--color-purple)`"
						>
							{{ row.type }}
						</TagItem>
						<TagItem
							v-else
							class="border !border-solid border-blue bg-highlight-blue !font-medium"
							:style="`--_color: var(--color-blue)`"
						>
							{{ row.type }}
						</TagItem>
					</template>
					<template #cell-name="{ row }">
						<span
							class="block cursor-pointer truncate pr-8 font-semibold"
							@click="copyText(row.name)"
						>
							{{ row.name }}
						</span>
					</template>
					<template #cell-content="{ row }">
						<span
							class="block cursor-pointer truncate pr-8 font-semibold"
							@click="copyText(row.content)"
						>
							{{ row.content }}
						</span>
					</template>
				</Table>

				<span>
					Set up your personal domain to connect to your server via custom DNS records.
				</span>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { UploadIcon } from '@modrinth/assets'
import { computed, ref } from 'vue'

import { ButtonStyled, StyledInput, Table, TagItem } from '#ui/components'
import type { TableColumn } from '#ui/components/base'
import { injectModrinthServerContext, injectNotificationManager } from '#ui/providers'

const { addNotification } = injectNotificationManager()
const { server } = injectModrinthServerContext()

const userDomain = ref('')
const exampleDomain = 'play.example.com'

const dnsColumns: TableColumn[] = [
	{ key: 'type', label: 'Type', width: '20%' },
	{ key: 'name', label: 'Name', width: '35%' },
	{ key: 'content', label: 'Content' },
]

const dnsRecords = computed(() => {
	const domain = userDomain.value === '' ? exampleDomain : userDomain.value
	return [
		{
			type: 'A',
			name: `${domain}`,
			content: server.value?.net?.ip ?? '',
		},
		{
			type: 'SRV',
			name: `_minecraft._tcp.${domain}`,
			content: `0 10 ${server.value?.net?.port} ${domain}`,
		},
	]
})

type DnsRecord = {
	type: string
	name: string
	content: string
}

const exportDnsRecords = () => {
	const records = dnsRecords.value.reduce(
		(acc, record) => {
			const type = record.type
			if (!acc[type]) {
				acc[type] = []
			}
			acc[type].push(record)
			return acc
		},
		{} as Record<string, DnsRecord[]>,
	)

	const text = Object.entries(records)
		.map(([type, records]) => {
			return `; ${type} Records\n${records.map((record) => `${record.name}.\t1\tIN\t${record.type} ${record.content}${record.type === 'SRV' ? '.' : ''}`).join('\n')}\n`
		})
		.join('\n')
	const blob = new Blob([text], { type: 'text/plain' })
	const a = document.createElement('a')
	a.href = window.URL.createObjectURL(blob)
	a.download = `${userDomain.value}.txt`
	a.click()
	a.remove()
}

const copyText = (text: string) => {
	navigator.clipboard.writeText(text)
	addNotification({
		type: 'success',
		title: 'Text copied',
		text: `${text} has been copied to your clipboard`,
	})
}
</script>
