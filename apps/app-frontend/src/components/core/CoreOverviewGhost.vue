<script setup lang="ts">
import { ChevronUpIcon, FilterIcon, SearchIcon, SettingsIcon, UserPlusIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	Combobox,
	type ComboboxOption,
	GhostBlock,
	GhostControl,
	GhostMedia,
	GhostTabGroup,
	GhostText,
	StyledInput,
	Table,
	type SortDirection,
	type TableColumn,
} from '@modrinth/ui'
import { ref } from 'vue'

defineOptions({
	name: 'CoreOverviewGhost',
})

type AccessTableColumn = 'user' | 'role' | 'joined' | 'actions'
type RoleFilter = 'all' | 'owner' | 'editor' | 'viewer'
type GhostMember = {
	id: string
	userWidth: string
	roleWidth: string
	joinedWidth: string
	showAction: boolean
}

const tabLabels = ['Overview', 'Activity']
const search = ref('')
const roleFilter = ref<RoleFilter>('all')
const sortColumn = ref<string | undefined>('role')
const sortDirection = ref<SortDirection>('asc')
const roleFilterOptions: ComboboxOption<RoleFilter>[] = [
	{ value: 'all', label: 'All roles' },
	{ value: 'owner', label: 'Owner' },
	{ value: 'editor', label: 'Admin' },
	{ value: 'viewer', label: 'Member' },
]
const columns: TableColumn<AccessTableColumn>[] = [
	{ key: 'user', label: 'User', width: '32%', enableSorting: true },
	{ key: 'role', label: 'Role', width: '28%', enableSorting: true },
	{ key: 'joined', label: 'Joined', enableSorting: true },
	{ key: 'actions', label: 'Actions', align: 'right', width: '7rem' },
]
const members: GhostMember[] = [
	{
		id: 'ghost-owner',
		userWidth: '7.5rem',
		roleWidth: '4.25rem',
		joinedWidth: '5rem',
		showAction: false,
	},
	{
		id: 'ghost-admin-1',
		userWidth: '9rem',
		roleWidth: '4.75rem',
		joinedWidth: '6rem',
		showAction: true,
	},
	{
		id: 'ghost-admin-2',
		userWidth: '6.75rem',
		roleWidth: '4.75rem',
		joinedWidth: '5.5rem',
		showAction: true,
	},
	{
		id: 'ghost-member-1',
		userWidth: '8.25rem',
		roleWidth: '5.5rem',
		joinedWidth: '6.25rem',
		showAction: true,
	},
	{
		id: 'ghost-member-2',
		userWidth: '7rem',
		roleWidth: '5.5rem',
		joinedWidth: '5.75rem',
		showAction: true,
	},
	{
		id: 'ghost-member-3',
		userWidth: '8.75rem',
		roleWidth: '5.5rem',
		joinedWidth: '6.5rem',
		showAction: true,
	},
	{
		id: 'ghost-member-4',
		userWidth: '6.25rem',
		roleWidth: '5.5rem',
		joinedWidth: '5.25rem',
		showAction: true,
	},
	{
		id: 'ghost-member-5',
		userWidth: '9.5rem',
		roleWidth: '5.5rem',
		joinedWidth: '6rem',
		showAction: true,
	},
]
</script>

<template>
	<div class="flex w-full flex-1 flex-col gap-4 pointer-events-none" inert aria-hidden="true">
		<div class="flex items-start justify-between gap-4">
			<div class="flex min-w-0 flex-wrap items-center gap-3">
				<GhostTabGroup :count="2" :labels="tabLabels" />
			</div>
			<div class="flex shrink-0 items-center gap-2">
				<div class="flex h-10 items-center gap-2 text-sm font-semibold text-secondary">
					<GhostBlock shape="circle" class="size-3" />
					<GhostText kind="metadata" width="100%" :style="{ width: '4rem' }" />
				</div>
				<ButtonStyled circular>
					<button type="button" class="!h-12 !w-12" tabindex="-1">
						<SettingsIcon class="!h-6 !w-6 text-secondary" />
					</button>
				</ButtonStyled>
			</div>
		</div>

		<div class="flex flex-col gap-4">
			<div class="flex flex-col gap-2 md:flex-row">
				<StyledInput
					v-model="search"
					:icon="SearchIcon"
					placeholder="Search 8 members..."
					wrapper-class="min-w-0 flex-1"
					input-class="!h-10"
					readonly
					clearable
				/>
				<div class="flex shrink-0 flex-wrap items-center gap-2 md:flex-nowrap">
					<Combobox
						v-model="roleFilter"
						:options="roleFilterOptions"
						display-value="All roles"
						trigger-class="min-w-[225px] !h-10 !min-h-10 !py-0"
					>
						<template #prefix><FilterIcon class="size-5 text-secondary" /></template>
					</Combobox>
					<ButtonStyled color="brand">
						<button type="button" class="!h-10 w-full md:w-fit" tabindex="-1">
							<UserPlusIcon />
							Invite friends
						</button>
					</ButtonStyled>
				</div>
			</div>

			<Table
				v-model:sort-column="sortColumn"
				v-model:sort-direction="sortDirection"
				class="hidden sm:block"
				:columns="columns"
				:data="members"
				row-key="id"
				table-min-width="42rem"
			>
				<template #cell-user="{ row: member }">
					<span class="inline-flex max-w-full min-w-0 items-center gap-2">
						<GhostMedia kind="circle" class="!w-[22px] shrink-0" />
						<GhostText
							kind="body"
							width="100%"
							class="min-w-0"
							:style="{ width: member.userWidth }"
						/>
					</span>
				</template>

				<template #cell-role="{ row: member }">
					<GhostBlock
						shape="pill"
						class="inline-flex h-7 max-w-full"
						:style="{ width: member.roleWidth }"
					/>
				</template>

				<template #cell-joined="{ row: member }">
					<GhostText
						kind="body"
						width="100%"
						class="inline-flex align-middle"
						:style="{ width: member.joinedWidth }"
					/>
				</template>

				<template #cell-actions="{ row: member }">
					<div v-if="member.showAction" class="flex items-center justify-end gap-1">
						<GhostControl kind="icon-button" size="small" />
					</div>
				</template>
			</Table>

			<div class="overflow-hidden rounded-2xl border border-solid border-surface-4 sm:hidden">
				<div
					class="grid min-h-14 grid-cols-[minmax(0,1.35fr)_7.75rem_minmax(6rem,0.8fr)_4rem] bg-surface-3"
				>
					<div class="flex items-center pl-4 font-semibold text-secondary">
						<button
							type="button"
							class="flex min-w-0 cursor-pointer items-center gap-1 border-none bg-transparent p-0 font-semibold text-secondary transition-colors"
							tabindex="-1"
						>
							<span class="min-w-0 truncate">User</span>
						</button>
					</div>
					<div class="flex items-center font-semibold text-secondary">
						<button
							type="button"
							class="flex cursor-pointer items-center gap-1 border-none bg-transparent p-0 font-semibold text-contrast transition-colors"
							tabindex="-1"
						>
							Role
							<ChevronUpIcon class="size-4" />
						</button>
					</div>
					<div class="flex items-center justify-end font-semibold text-secondary">
						<button
							type="button"
							class="flex cursor-pointer items-center gap-1 border-none bg-transparent p-0 font-semibold text-secondary transition-colors"
							tabindex="-1"
						>
							Joined
						</button>
					</div>
					<div class="flex items-center justify-end pr-4 font-semibold text-secondary">
						<span class="sr-only">Actions</span>
					</div>
				</div>
				<div
					v-for="(member, index) in members"
					:key="member.id"
					class="grid min-h-16 grid-cols-[minmax(0,1.35fr)_7.75rem_minmax(6rem,0.8fr)_4rem] items-center border-0 border-t border-solid border-surface-4"
					:class="index % 2 === 0 ? 'bg-surface-2' : 'bg-surface-1.5'"
				>
					<div class="flex min-w-0 items-center pl-4">
						<span class="inline-flex min-w-0 items-center gap-2">
							<GhostMedia kind="circle" class="!w-6 shrink-0" />
							<GhostText
								kind="body"
								width="100%"
								class="min-w-0"
								:style="{ width: member.userWidth }"
							/>
						</span>
					</div>
					<div class="min-w-0 py-3 pr-2">
						<GhostBlock
							shape="pill"
							class="inline-flex h-7 max-w-full"
							:style="{ width: member.roleWidth }"
						/>
					</div>
					<div class="min-w-0 py-3 pr-2 text-right text-secondary">
						<GhostText
							kind="body"
							width="100%"
							class="inline-flex max-w-full align-middle"
							:style="{ width: member.joinedWidth }"
						/>
					</div>
					<div class="flex min-w-0 items-center justify-end pr-4">
						<GhostControl v-if="member.showAction" kind="icon-button" size="small" />
					</div>
				</div>
			</div>
		</div>
	</div>
</template>
