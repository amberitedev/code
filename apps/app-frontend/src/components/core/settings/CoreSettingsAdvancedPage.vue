<script setup lang="ts">
import { SettingsLabel, StyledInput, Toggle, UnsavedChangesPopup } from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, ref } from 'vue'

const savedSettings = useStorage('copal-advanced-settings', {
	auditRetentionDays: 90,
	autoApproveFriends: false,
	requireKnownAccounts: true,
})
const auditRetentionDays = ref(savedSettings.value.auditRetentionDays)
const autoApproveFriends = ref(savedSettings.value.autoApproveFriends)
const requireKnownAccounts = ref(savedSettings.value.requireKnownAccounts)
const isUpdating = ref(false)
const savedAdvanced = computed(() => ({
	...savedSettings.value,
}))
const modifiedAdvanced = computed(() => ({
	auditRetentionDays: auditRetentionDays.value,
	autoApproveFriends: autoApproveFriends.value,
	requireKnownAccounts: requireKnownAccounts.value,
}))

function saveAdvanced() {
	isUpdating.value = true
	savedSettings.value = { ...modifiedAdvanced.value }
	isUpdating.value = false
}

function resetAdvanced() {
	auditRetentionDays.value = savedSettings.value.auditRetentionDays
	autoApproveFriends.value = savedSettings.value.autoApproveFriends
	requireKnownAccounts.value = savedSettings.value.requireKnownAccounts
}
</script>

<template>
	<div class="relative h-full w-full">
		<div class="flex flex-col gap-6 pb-10">
			<section class="flex flex-col gap-3">
				<SettingsLabel
					title="Advanced"
					description="Core-wide defaults that affect group access behavior."
				/>
				<div class="flex items-center justify-between gap-4">
					<label class="flex flex-col gap-1">
						<span class="text-lg font-semibold text-contrast">Auto-approve friends</span>
						<span class="text-sm text-secondary">
							Allow existing friends to join with an invite without extra approval.
						</span>
					</label>
					<Toggle v-model="autoApproveFriends" />
				</div>
				<div class="flex items-center justify-between gap-4">
					<label class="flex flex-col gap-1">
						<span class="text-lg font-semibold text-contrast">Require known accounts</span>
						<span class="text-sm text-secondary">
							Only allow members that have completed Amberite account setup.
						</span>
					</label>
					<Toggle v-model="requireKnownAccounts" />
				</div>
			</section>

			<section class="flex max-w-[500px] flex-col gap-2.5">
				<SettingsLabel
					id="audit-retention-days"
					title="Audit retention"
					description="How many days of member and permission activity to keep."
				/>
				<StyledInput
					id="audit-retention-days"
					v-model="auditRetentionDays"
					type="number"
					:min="7"
					:max="365"
				/>
			</section>
		</div>
		<UnsavedChangesPopup
			:original="savedAdvanced"
			:modified="modifiedAdvanced"
			:saving="isUpdating"
			@save="saveAdvanced"
			@reset="resetAdvanced"
		/>
	</div>
</template>
