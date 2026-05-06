import { DownloadIcon, EditIcon, InfoIcon, MoreVerticalIcon, SearchIcon, SettingsIcon, TrashIcon } from '@modrinth/assets'
import type { StoryObj } from '@storybook/vue3-vite'
import { defineComponent, h, onMounted, ref } from 'vue'

// ── Base ─────────────────────────────────────────────────────────────────────
import Accordion from '../components/base/Accordion.vue'
import Admonition from '../components/base/Admonition.vue'
import AppearingProgressBar from '../components/base/AppearingProgressBar.vue'
import AutoBrandIcon from '../components/base/AutoBrandIcon.vue'
import AutoLink from '../components/base/AutoLink.vue'
import Avatar from '../components/base/Avatar.vue'
import Badge from '../components/base/Badge.vue'
import BaseTerminal from '../components/base/BaseTerminal.vue'
import BulletDivider from '../components/base/BulletDivider.vue'
import Button from '../components/base/Button.vue'
import ButtonStyled from '../components/base/ButtonStyled.vue'
import Card from '../components/base/Card.vue'
import Checkbox from '../components/base/Checkbox.vue'
import Chips from '../components/base/Chips.vue'
import Collapsible from '../components/base/Collapsible.vue'
import CollapsibleRegion from '../components/base/CollapsibleRegion.vue'
// @ts-ignore
import Combobox from '../components/base/Combobox.vue'
import ContentPageHeader from '../components/base/ContentPageHeader.vue'
import CopyCode from '../components/base/CopyCode.vue'
import DoubleIcon from '../components/base/DoubleIcon.vue'
import DropArea from '../components/base/DropArea.vue'
import DropdownSelect from '../components/base/DropdownSelect.vue'
import DropzoneFileInput from '../components/base/DropzoneFileInput.vue'
import EmptyState from '../components/base/EmptyState.vue'
import EnvironmentIndicator from '../components/base/EnvironmentIndicator.vue'
import ErrorInformationCard from '../components/base/ErrorInformationCard.vue'
import FileInput from '../components/base/FileInput.vue'
import FilterBar from '../components/base/FilterBar.vue'
import HeadingLink from '../components/base/HeadingLink.vue'
import HorizontalRule from '../components/base/HorizontalRule.vue'
import JoinedButtons from '../components/base/JoinedButtons.vue'
import LargeRadioButton from '../components/base/LargeRadioButton.vue'
import LoadingBar from '../components/base/LoadingBar.vue'
import LoadingIndicator from '../components/base/LoadingIndicator.vue'
import MarkdownEditor from '../components/base/MarkdownEditor.vue'
// @ts-ignore
import MultiSelect from '../components/base/MultiSelect.vue'
import NavTabs from '../components/base/NavTabs.vue'
import OptionGroup from '../components/base/OptionGroup.vue'
import OverflowMenu from '../components/base/OverflowMenu.vue'
import Pagination from '../components/base/Pagination.vue'
import PopoutMenu from '../components/base/PopoutMenu.vue'
import PreviewSelectButton from '../components/base/PreviewSelectButton.vue'
import ProgressBar from '../components/base/ProgressBar.vue'
import ProgressSpinner from '../components/base/ProgressSpinner.vue'
import RadialHeader from '../components/base/RadialHeader.vue'
import RadioButtons from '../components/base/RadioButtons.vue'
import ReadyTransition from '../components/base/ReadyTransition.vue'
import ScrollablePanel from '../components/base/ScrollablePanel.vue'
import ServerNotice from '../components/base/ServerNotice.vue'
import SettingsLabel from '../components/base/SettingsLabel.vue'
import SimpleBadge from '../components/base/SimpleBadge.vue'
import Slider from '../components/base/Slider.vue'
import SmartClickable from '../components/base/SmartClickable.vue'
import StackedAdmonitions from '../components/base/StackedAdmonitions.vue'
import StyledInput from '../components/base/StyledInput.vue'
// @ts-ignore
import Table from '../components/base/Table.vue'
import TagItem from '../components/base/TagItem.vue'
import Timeline from '../components/base/Timeline.vue'
import Toggle from '../components/base/Toggle.vue'
// @ts-ignore
import UnsavedChangesPopup from '../components/base/UnsavedChangesPopup.vue'

// ── Modal ─────────────────────────────────────────────────────────────────────
import ConfirmLeaveModal from '../components/modal/ConfirmLeaveModal.vue'
import NewModal from '../components/modal/NewModal.vue'
import ShareModal from '../components/modal/ShareModal.vue'
import TabbedModal from '../components/modal/TabbedModal.vue'
import UploadProgressModal from '../components/modal/UploadProgressModal.vue'

// ── Project ───────────────────────────────────────────────────────────────────
import ProjectCard from '../components/project/card/ProjectCard.vue'
import ProjectCombobox from '../components/project/ProjectCombobox.vue'
import ProjectSidebarCompatibility from '../components/project/ProjectSidebarCompatibility.vue'
import ProjectSidebarCreators from '../components/project/ProjectSidebarCreators.vue'
import ProjectSidebarDetails from '../components/project/ProjectSidebarDetails.vue'
import ProjectSidebarLinks from '../components/project/ProjectSidebarLinks.vue'
import ProjectSidebarServerInfo from '../components/project/ProjectSidebarServerInfo.vue'
import ProjectSidebarTags from '../components/project/ProjectSidebarTags.vue'

// ── Servers ───────────────────────────────────────────────────────────────────
import InstallingBanner from '../components/servers/InstallingBanner.vue'
import SaveBanner from '../components/servers/SaveBanner.vue'
import ServerListing from '../components/servers/ServerListing.vue'

// ── Instances ─────────────────────────────────────────────────────────────────
import ContentModpackCard from '../layouts/shared/content-tab/components/ContentModpackCard.vue'

export default {
	title: 'Overview/All Components',
	parameters: {
		layout: 'padded',
		controls: { disable: true },
	},
}

export const AllComponents: StoryObj = {
	render: () => ({
		components: {
			// Base
			Accordion,
			Admonition,
			AppearingProgressBar,
			AutoBrandIcon,
			AutoLink,
			Avatar,
			Badge,
			BaseTerminal,
			BulletDivider,
			Button,
			ButtonStyled,
			Card,
			Checkbox,
			Chips,
			Collapsible,
			CollapsibleRegion,
			Combobox,
			ContentPageHeader,
			CopyCode,
			DoubleIcon,
			DropArea,
			DropdownSelect,
			DropzoneFileInput,
			EmptyState,
			EnvironmentIndicator,
			ErrorInformationCard,
			FileInput,
			FilterBar,
			HeadingLink,
			HorizontalRule,
			JoinedButtons,
			LargeRadioButton,
			LoadingBar,
			LoadingIndicator,
			MarkdownEditor,
			MultiSelect,
			NavTabs,
			OptionGroup,
			OverflowMenu,
			Pagination,
			PopoutMenu,
			PreviewSelectButton,
			ProgressBar,
			ProgressSpinner,
			RadialHeader,
			RadioButtons,
			ReadyTransition,
			ScrollablePanel,
			ServerNotice,
			SettingsLabel,
			SimpleBadge,
			Slider,
			SmartClickable,
			StackedAdmonitions,
			StyledInput,
			Table,
			TagItem,
			Timeline,
			Toggle,
			UnsavedChangesPopup,
			// Modal
			ConfirmLeaveModal,
			NewModal,
			ShareModal,
			TabbedModal,
			UploadProgressModal,
			// Project
			ProjectCard,
			ProjectCombobox,
			ProjectSidebarCompatibility,
			ProjectSidebarCreators,
			ProjectSidebarDetails,
			ProjectSidebarLinks,
			ProjectSidebarServerInfo,
			ProjectSidebarTags,
			// Servers
			InstallingBanner,
			SaveBanner,
			ServerListing,
			// Instances
			ContentModpackCard,
		},
		setup() {
			// ── Reactive state ─────────────────────────────────────
			const toggleValue = ref(true)
			const checkboxValue = ref(true)
			const sliderValue = ref(65)
			const inputValue = ref('')
			const markdownValue = ref('**Bold**, *italic*, and `code`.\n\nA [link](https://modrinth.com).')
			const navTabIndex = ref(0)
			const radioValue = ref('fabric')
			const multiSelectValue = ref<string[]>(['fabric'])
			const dropdownValue = ref('grid')
			const chipsValue = ref<string[]>(['optimization'])
			const filterSelected = ref<string[]>(['mods'])

			// ── Modal refs ─────────────────────────────────────────
			const newModalRef = ref<InstanceType<typeof NewModal> | null>(null)
			const shareModalRef = ref<InstanceType<typeof ShareModal> | null>(null)
			const tabbedModalRef = ref<InstanceType<typeof TabbedModal> | null>(null)
			const uploadModalRef = ref<InstanceType<typeof UploadProgressModal> | null>(null)
			const confirmLeaveModalRef = ref<InstanceType<typeof ConfirmLeaveModal> | null>(null)

			// ── TabbedModal tab data ───────────────────────────────
			const makeTab = (label: string) =>
				defineComponent({ render: () => h('p', { class: 'text-secondary' }, `${label} tab content.`) })
			const tabbedModalTabs = [
				{ name: { id: 'general', defaultMessage: 'General' }, icon: InfoIcon, content: makeTab('General') },
				{ name: { id: 'appearance', defaultMessage: 'Appearance' }, icon: SettingsIcon, content: makeTab('Appearance') },
				{ name: { id: 'advanced', defaultMessage: 'Advanced' }, icon: EditIcon, content: makeTab('Advanced') },
			]

			// ── Terminal ref ──────────────────────────────────────
			const termRef = ref<InstanceType<typeof BaseTerminal> | null>(null)
			onMounted(() => {
				termRef.value?.writeln('\x1b[1;32m=== Server Console ===\x1b[0m')
				termRef.value?.writeln(
					'\x1b[36m[10:15:30]\x1b[0m \x1b[32m[Server/INFO]\x1b[0m: Loading properties',
				)
				termRef.value?.writeln(
					'\x1b[36m[10:15:31]\x1b[0m \x1b[32m[Server/INFO]\x1b[0m: Starting on *:25565',
				)
				termRef.value?.writeln(
					'\x1b[36m[10:15:33]\x1b[0m \x1b[33m[Server/WARN]\x1b[0m: Server may be overloaded',
				)
				termRef.value?.writeln(
					'\x1b[36m[10:15:34]\x1b[0m \x1b[32m[Server/INFO]\x1b[0m: Done! For help, type "help"',
				)
			})

			// ── Table data ─────────────────────────────────────────
			const tableColumns = [
				{ key: 'name', label: 'Name', enableSorting: true },
				{ key: 'role', label: 'Role' },
				{ key: 'status', label: 'Status' },
			]
			const tableData = [
				{ name: 'Alice Johnson', role: 'Owner', status: 'active' },
				{ name: 'Bob Smith', role: 'Developer', status: 'active' },
				{ name: 'Carol White', role: 'Artist', status: 'inactive' },
			]

			// ── Project Card data ──────────────────────────────────
			const projectCardData = {
				link: '/mod/sodium',
				layout: 'grid',
				title: 'Sodium',
				author: { name: 'JellySquid3', link: '/user/JellySquid3' },
				summary: 'A modern rendering engine and client-side optimization mod for Minecraft.',
				iconUrl: 'https://cdn.modrinth.com/data/AANobbMI/icon.png',
				downloads: 30000000,
				followers: 45000,
				dateUpdated: '2024-12-01T00:00:00Z',
				tags: ['optimization', 'fabric'],
				environment: { clientSide: 'required', serverSide: 'optional' },
			}

			// ── Sidebar data ───────────────────────────────────────
			const sidebarProject = {
				id: 'demo-project',
				published: new Date(Date.now() - 365 * 86400000).toISOString(),
				updated: new Date(Date.now() - 3 * 86400000).toISOString(),
				approved: new Date(Date.now() - 360 * 86400000).toISOString(),
				queued: '',
				status: 'approved',
				license: { id: 'LGPL-3.0-only', url: 'https://www.gnu.org/licenses/lgpl-3.0.html' },
			}
			const sidebarMembers = [
				{
					id: 'm1',
					role: 'Owner',
					is_owner: true,
					accepted: true,
					user: {
						id: 'u1',
						username: 'jellysquid3',
						avatar_url: 'https://avatars.githubusercontent.com/u/31803019?v=4',
					},
				},
				{
					id: 'm2',
					role: 'Developer',
					is_owner: false,
					accepted: true,
					user: { id: 'u2', username: 'modder42', avatar_url: '' },
				},
			]
			const sidebarLinks = {
				issues_url: 'https://github.com/example/mod/issues',
				source_url: 'https://github.com/example/mod',
				wiki_url: 'https://wiki.example.com',
				discord_url: 'https://discord.gg/example',
				donation_urls: [{ id: 'patreon', url: 'https://patreon.com/example' }],
			}

			// ── Sidebar compatibility data ─────────────────────────
			const compatibilityTags = {
				gameVersions: [
					{ version: '1.21.4', version_type: 'release', date: '2024-12-03', major: true },
					{ version: '1.21.1', version_type: 'release', date: '2024-08-08', major: true },
					{ version: '1.20.4', version_type: 'release', date: '2023-12-07', major: true },
					{ version: '1.20.1', version_type: 'release', date: '2023-06-12', major: true },
				],
				loaders: [
					{ icon: '', name: 'fabric', supported_project_types: ['mod'] },
					{ icon: '', name: 'forge', supported_project_types: ['mod'] },
					{ icon: '', name: 'neoforge', supported_project_types: ['mod'] },
				],
			}
			const compatibilityProject = {
				actualProjectType: 'mod',
				project_type: 'mod',
				loaders: ['fabric', 'forge', 'neoforge'],
				client_side: 'required',
				server_side: 'optional',
				versions: [{ game_versions: ['1.21.4', '1.21.1', '1.20.4', '1.20.1'] }],
			}

			// ── Server info data ───────────────────────────────────
			const serverInfoProject = {
				minecraft_java_server: {
					address: 'play.example.com',
					content: {
						kind: 'vanilla',
						recommended_game_version: '1.21.4',
						supported_game_versions: ['1.21.4', '1.21.1'],
					},
					ping: null,
				},
				minecraft_server: {
					country: 'US',
					languages: ['English'],
				},
			}

			// ── Server data ────────────────────────────────────────
			const serverData = {
				server_id: 'srv-demo-001',
				name: 'Survival SMP',
				status: 'available',
				game: 'Minecraft',
				mc_version: '1.21.4',
				loader: 'Fabric',
				loader_version: '0.16.14',
				net: { ip: '203.0.113.1', port: 25565, domain: 'play' },
			}

			// ── Modpack data ───────────────────────────────────────
			const modpackProject = {
				id: '1KVo5zza',
				slug: 'fabulously-optimized',
				title: 'Fabulously Optimized',
				icon_url: 'https://cdn.modrinth.com/data/1KVo5zza/9f1ded4949c2a9db5ca382d3bcc912c7245486b4_96.webp',
				description: 'Beautiful graphics, speedy performance and familiar features.',
				downloads: 8700000,
				followers: 3762,
			}
			const modpackVersion = {
				id: 'YEEXo8mO',
				version_number: '1.12.1',
				date_published: '2024-02-10T06:53:28Z',
			}
			const modpackOwner = {
				id: '2avTeeAE',
				name: 'robotkoer',
				avatar_url: 'https://cdn.modrinth.com/user/2avTeeAE/icon.png',
				type: 'user',
			}

			// ── Filter options ─────────────────────────────────────
			const filterOptions = [
				{ id: 'mods', message: { id: 'filter.mods', defaultMessage: 'Mods' } },
				{ id: 'plugins', message: { id: 'filter.plugins', defaultMessage: 'Plugins' } },
				{ id: 'modpacks', message: { id: 'filter.modpacks', defaultMessage: 'Modpacks' } },
				{
					id: 'resourcepacks',
					message: { id: 'filter.resourcepacks', defaultMessage: 'Resource Packs' },
				},
			]

			return {
				// state
				toggleValue,
				checkboxValue,
				sliderValue,
				inputValue,
				markdownValue,
				navTabIndex,
				radioValue,
				multiSelectValue,
				dropdownValue,
				chipsValue,
				filterSelected,
				filterOptions,
				// modal refs
				newModalRef,
				shareModalRef,
				tabbedModalRef,
				uploadModalRef,
				confirmLeaveModalRef,
				tabbedModalTabs,
				// terminal
				termRef,
				// table
				tableColumns,
				tableData,
				// component data
				projectCardData,
				sidebarProject,
				sidebarMembers,
				sidebarLinks,
				compatibilityTags,
				compatibilityProject,
				serverInfoProject,
				serverData,
				modpackProject,
				modpackVersion,
				modpackOwner,
				// icons
				SearchIcon,
				DownloadIcon,
				EditIcon,
				MoreVerticalIcon,
				SettingsIcon,
				TrashIcon,
			}
		},
		template: /*html*/ `
<div class="p-8 space-y-16 bg-bg">

  <!-- PAGE HEADER -->
  <div class="space-y-2">
    <h1 class="text-4xl font-extrabold text-contrast tracking-tight">Component Library</h1>
    <p class="text-lg text-secondary">Every component in <code class="text-brand font-mono">@modrinth/ui</code> — name, description, and live demo. Ordered most-common → most-specialized.</p>
  </div>

  <!-- ══════════════════════════════════════════ -->
  <!--  1. BUTTONS & ACTIONS                     -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Buttons &amp; Actions</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Button</code><span class="text-xs text-secondary ml-2">— Standard Modrinth button with color variants</span></div>
        <div class="flex flex-wrap gap-2">
          <Button color="default">Default</Button>
          <Button color="primary">Primary</Button>
          <Button color="green">Green</Button>
          <Button color="danger">Danger</Button>
          <Button color="default" :large="true">Large</Button>
          <Button color="default" :disabled="true">Disabled</Button>
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ButtonStyled</code><span class="text-xs text-secondary ml-2">— Configurable styled button: colors, types, sizes, icons</span></div>
        <div class="flex flex-wrap gap-2 items-center">
          <ButtonStyled color="brand"><button>Brand</button></ButtonStyled>
          <ButtonStyled color="brand" type="outlined"><button>Outlined</button></ButtonStyled>
          <ButtonStyled color="red" type="transparent"><button>Transparent</button></ButtonStyled>
          <ButtonStyled color="brand" type="chip"><button>Chip</button></ButtonStyled>
          <ButtonStyled color="brand" type="highlight"><button>Highlight</button></ButtonStyled>
          <ButtonStyled color="brand" circular><button><DownloadIcon /></button></ButtonStyled>
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">JoinedButtons</code><span class="text-xs text-secondary ml-2">— Visually joined button group</span></div>
        <JoinedButtons>
          <ButtonStyled><button>Left</button></ButtonStyled>
          <ButtonStyled><button>Middle</button></ButtonStyled>
          <ButtonStyled><button>Right</button></ButtonStyled>
        </JoinedButtons>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">OverflowMenu</code><span class="text-xs text-secondary ml-2">— Click to reveal overflow actions in a dropdown</span></div>
        <ButtonStyled circular type="transparent">
          <OverflowMenu :options="[{ id: 'edit', action: () => {} }, { divider: true }, { id: 'delete', color: 'red', action: () => {} }]" aria-label="More options">
            <MoreVerticalIcon />
            <template #edit><EditIcon class="w-4 h-4" /> Edit</template>
            <template #delete><TrashIcon class="w-4 h-4" /> Delete</template>
          </OverflowMenu>
        </ButtonStyled>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">PopoutMenu</code><span class="text-xs text-secondary ml-2">— Floating popout triggered by its default-slot content</span></div>
        <ButtonStyled circular type="transparent">
          <PopoutMenu>
            <SettingsIcon class="w-5 h-5" />
            <template #menu>
              <div class="flex flex-col gap-1 p-1">
                <Button transparent>Profile</Button>
                <Button transparent>Settings</Button>
                <Button color="danger" transparent>Sign out</Button>
              </div>
            </template>
          </PopoutMenu>
        </ButtonStyled>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">SmartClickable</code><span class="text-xs text-secondary ml-2">— Makes an entire card region clickable via an inner link</span></div>
        <SmartClickable>
          <template #clickable>
            <a href="#" style="display:block;position:absolute;inset:0;" aria-label="View project"></a>
          </template>
          <div class="relative p-4 bg-bg-raised rounded-lg border border-surface cursor-pointer hover:border-brand transition-colors">
            <p class="font-semibold text-contrast">Sodium</p>
            <p class="text-secondary text-sm">The entire card is one big clickable link.</p>
          </div>
        </SmartClickable>
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  2. INPUTS & FORMS                        -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Inputs &amp; Forms</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">StyledInput</code><span class="text-xs text-secondary ml-2">— Text input with icon, clear button, and textarea mode</span></div>
        <div class="space-y-2">
          <StyledInput v-model="inputValue" :icon="SearchIcon" placeholder="Search mods..." clearable />
          <StyledInput multiline placeholder="Enter a description..." />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Toggle</code><span class="text-xs text-secondary ml-2">— On/off toggle switch with small size variant</span></div>
        <div class="flex flex-col gap-3">
          <div class="flex items-center gap-3"><Toggle v-model="toggleValue" /><span class="text-sm text-secondary">{{ toggleValue ? 'Enabled' : 'Disabled' }}</span></div>
          <div class="flex items-center gap-3"><Toggle :model-value="false" :small="true" /><span class="text-sm text-secondary">Small</span></div>
          <div class="flex items-center gap-3"><Toggle :model-value="false" :disabled="true" /><span class="text-sm text-secondary">Disabled</span></div>
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Checkbox</code><span class="text-xs text-secondary ml-2">— Checkbox with label; supports indeterminate state</span></div>
        <div class="flex flex-col gap-2">
          <Checkbox v-model="checkboxValue" label="Checked state" />
          <Checkbox :model-value="false" :indeterminate="true" label="Indeterminate" />
          <Checkbox :model-value="false" label="Disabled" :disabled="true" />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Slider</code><span class="text-xs text-secondary ml-2">— Range slider with optional unit and snap points</span></div>
        <Slider v-model="sliderValue" :min="0" :max="100" unit="%" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">RadioButtons</code><span class="text-xs text-secondary ml-2">— Radio button group from an options array</span></div>
        <RadioButtons v-model="radioValue" :options="[{ label: 'Fabric', value: 'fabric' }, { label: 'Forge', value: 'forge' }, { label: 'NeoForge', value: 'neoforge' }]" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">LargeRadioButton</code><span class="text-xs text-secondary ml-2">— Large visual radio button for prominent option selection</span></div>
        <div class="flex gap-3">
          <LargeRadioButton v-model="radioValue" value="fabric" label="Fabric" />
          <LargeRadioButton v-model="radioValue" value="forge" label="Forge" />
          <LargeRadioButton v-model="radioValue" value="neoforge" label="NeoForge" />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">DropdownSelect</code><span class="text-xs text-secondary ml-2">— Styled native select dropdown</span></div>
        <DropdownSelect v-model="dropdownValue" :options="[{ label: 'Grid', value: 'grid' }, { label: 'List', value: 'list' }, { label: 'Compact', value: 'compact' }]" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Combobox</code><span class="text-xs text-secondary ml-2">— Searchable dropdown combo box</span></div>
        <Combobox :options="[{ value: 'sodium', label: 'Sodium' }, { value: 'lithium', label: 'Lithium' }, { value: 'iris', label: 'Iris Shaders' }]" trigger-text="Select a mod..." searchable search-placeholder="Search mods..." />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">MultiSelect</code><span class="text-xs text-secondary ml-2">— Multi-select dropdown with checkboxes</span></div>
        <MultiSelect v-model="multiSelectValue" :options="[{ label: 'Fabric', value: 'fabric' }, { label: 'Forge', value: 'forge' }, { label: 'NeoForge', value: 'neoforge' }, { label: 'Quilt', value: 'quilt' }]" placeholder="Select loaders..." />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">OptionGroup</code><span class="text-xs text-secondary ml-2">— Segmented option selector</span></div>
        <OptionGroup v-model="radioValue" :options="[{ label: 'Fabric', value: 'fabric' }, { label: 'Forge', value: 'forge' }, { label: 'NeoForge', value: 'neoforge' }]" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Chips</code><span class="text-xs text-secondary ml-2">— Tag/chip multi-selector from a string array</span></div>
        <Chips v-model="chipsValue" :items="['optimization', 'adventure', 'fabric', 'forge', 'lightweight']" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">FilterBar</code><span class="text-xs text-secondary ml-2">— Horizontal filter chip selector with i18n labels</span></div>
        <FilterBar v-model="filterSelected" :options="filterOptions" show-all-options />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">FileInput</code><span class="text-xs text-secondary ml-2">— Native file picker input</span></div>
        <FileInput label="Upload icon" accept="image/png,image/jpeg,image/webp" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">DropzoneFileInput</code><span class="text-xs text-secondary ml-2">— File input with drag-and-drop zone</span></div>
        <DropzoneFileInput accept=".jar,.zip" primary-prompt="Drop mod files here" secondary-prompt="Accepts .jar and .zip up to 25 MB" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">DropArea</code><span class="text-xs text-secondary ml-2">— Full-page drop overlay activated when files are dragged onto the window</span></div>
        <DropArea accept="*" @change="() => {}">
          <div class="p-5 border-2 border-dashed border-secondary rounded-lg text-center">
            <p class="text-secondary text-sm">Drag files over the page to activate the overlay</p>
          </div>
        </DropArea>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">SettingsLabel</code><span class="text-xs text-secondary ml-2">— Label wrapper for settings form fields</span></div>
        <SettingsLabel title="Display name" description="Your public display name visible to other users.">
          <StyledInput placeholder="Enter your display name..." />
        </SettingsLabel>
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  3. CARDS & CONTAINERS                    -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Cards &amp; Containers</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Card</code><span class="text-xs text-secondary ml-2">— Content card container with optional collapsible header</span></div>
        <Card>
          <template #header><span class="font-semibold">Card Title</span></template>
          <p class="text-secondary text-sm">Card body content goes here.</p>
        </Card>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Accordion</code><span class="text-xs text-secondary ml-2">— Expandable/collapsible section with an animated toggle</span></div>
        <Accordion open-by-default>
          <template #title>Click to collapse</template>
          <p class="text-secondary text-sm mt-2">This content appears when the accordion is expanded.</p>
        </Accordion>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Collapsible</code><span class="text-xs text-secondary ml-2">— Animated height-collapse wrapper</span></div>
        <Collapsible :collapsed="false">
          <div class="p-3 bg-bg-raised rounded text-secondary text-sm">This content is visible when not collapsed.</div>
        </Collapsible>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">CollapsibleRegion</code><span class="text-xs text-secondary ml-2">— Accessible collapsible region with ARIA support</span></div>
        <CollapsibleRegion :open="true">
          <p class="text-secondary text-sm p-2">Collapsible region content.</p>
        </CollapsibleRegion>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ScrollablePanel</code><span class="text-xs text-secondary ml-2">— Panel with scrollable content and fade edges</span></div>
        <ScrollablePanel style="max-height: 100px;">
          <div class="space-y-1 p-2">
            <p v-for="i in 8" :key="i" class="text-secondary text-sm py-1 border-b border-surface">Row {{ i }}</p>
          </div>
        </ScrollablePanel>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">RadialHeader</code><span class="text-xs text-secondary ml-2">— Header section with a radial gradient background</span></div>
        <RadialHeader class="p-8">
          <h3 class="text-xl font-bold text-contrast text-center">Sodium</h3>
          <p class="text-secondary text-sm text-center">A rendering engine for Minecraft</p>
        </RadialHeader>
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  4. BADGES, TAGS & LABELS                 -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Badges, Tags &amp; Labels</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Badge</code><span class="text-xs text-secondary ml-2">— Status and role badges for projects and users</span></div>
        <div class="flex flex-wrap gap-2">
          <Badge type="approved" />
          <Badge type="processing" />
          <Badge type="draft" />
          <Badge type="rejected" />
          <Badge type="admin" />
          <Badge type="moderator" />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">SimpleBadge</code><span class="text-xs text-secondary ml-2">— Colored inline badge/pill label</span></div>
        <div class="flex gap-2 flex-wrap">
          <SimpleBadge formatted-name="New" color="green" />
          <SimpleBadge formatted-name="Updated" color="blue" />
          <SimpleBadge formatted-name="Beta" color="orange" />
          <SimpleBadge formatted-name="Deprecated" color="red" />
          <SimpleBadge formatted-name="Stable" color="gray" />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">TagItem</code><span class="text-xs text-secondary ml-2">— Tag chip with optional click action</span></div>
        <div class="flex flex-wrap gap-2">
          <TagItem>Optimization</TagItem>
          <TagItem>Adventure</TagItem>
          <TagItem>Technology</TagItem>
          <TagItem :action="() => {}">Clickable</TagItem>
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">BulletDivider</code><span class="text-xs text-secondary ml-2">— Bullet separator between inline metadata items</span></div>
        <div class="flex items-center gap-2 text-secondary text-sm">
          <span>30M downloads</span><BulletDivider /><span>45K followers</span><BulletDivider /><span>Updated 2 days ago</span>
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">HorizontalRule</code><span class="text-xs text-secondary ml-2">— Styled horizontal divider line</span></div>
        <HorizontalRule />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">HeadingLink</code><span class="text-xs text-secondary ml-2">— Heading with a hover-reveal anchor link</span></div>
        <HeadingLink id="demo-heading" level="2">Section Title</HeadingLink>
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  5. MEDIA & ICONS                         -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Media &amp; Icons</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Avatar</code><span class="text-xs text-secondary ml-2">— User or project avatar image with fallback</span></div>
        <div class="flex gap-4 items-center">
          <div class="flex flex-col items-center gap-1"><Avatar src="https://cdn.modrinth.com/data/AANobbMI/icon.png" size="2rem" /><span class="text-xs text-secondary">square</span></div>
          <div class="flex flex-col items-center gap-1"><Avatar src="https://cdn.modrinth.com/data/AANobbMI/icon.png" size="2.5rem" circle /><span class="text-xs text-secondary">circle</span></div>
          <div class="flex flex-col items-center gap-1"><Avatar size="2.5rem" /><span class="text-xs text-secondary">fallback</span></div>
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">AutoBrandIcon</code><span class="text-xs text-secondary ml-2">— Auto-selects the right brand icon from a keyword</span></div>
        <div class="flex gap-4 items-center">
          <div class="flex flex-col items-center gap-1"><AutoBrandIcon keyword="discord" class="w-7 h-7" /><span class="text-xs text-secondary">discord</span></div>
          <div class="flex flex-col items-center gap-1"><AutoBrandIcon keyword="github" class="w-7 h-7" /><span class="text-xs text-secondary">github</span></div>
          <div class="flex flex-col items-center gap-1"><AutoBrandIcon keyword="youtube" class="w-7 h-7" /><span class="text-xs text-secondary">youtube</span></div>
          <div class="flex flex-col items-center gap-1"><AutoBrandIcon keyword="patreon" class="w-7 h-7" /><span class="text-xs text-secondary">patreon</span></div>
          <div class="flex flex-col items-center gap-1"><AutoBrandIcon keyword="modrinth" class="w-7 h-7" /><span class="text-xs text-secondary">modrinth</span></div>
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">DoubleIcon</code><span class="text-xs text-secondary ml-2">— Two overlapping icons (primary + secondary badge)</span></div>
        <DoubleIcon>
          <template #primary><DownloadIcon class="w-6 h-6" /></template>
          <template #secondary><EditIcon class="w-3 h-3" /></template>
        </DoubleIcon>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">AutoLink</code><span class="text-xs text-secondary ml-2">— Smart link handling internal/external URLs</span></div>
        <div class="flex gap-4">
          <AutoLink href="/mod/sodium">Internal link</AutoLink>
          <AutoLink href="https://modrinth.com" external>External link ↗</AutoLink>
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">PreviewSelectButton</code><span class="text-xs text-secondary ml-2">— Selectable button with a visual preview thumbnail</span></div>
        <div class="flex gap-2">
          <PreviewSelectButton :checked="true">
            <template #preview><div class="w-16 h-16 bg-brand-highlight rounded-lg border border-brand" /></template>
            Dark
          </PreviewSelectButton>
          <PreviewSelectButton :checked="false">
            <template #preview><div class="w-16 h-16 bg-white rounded-lg border border-divider" /></template>
            Light
          </PreviewSelectButton>
          <PreviewSelectButton :checked="false">
            <template #preview><div class="w-16 h-16 bg-black rounded-lg border border-divider" /></template>
            OLED
          </PreviewSelectButton>
        </div>
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  6. FEEDBACK & NOTIFICATIONS              -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Feedback &amp; Notifications</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Admonition</code><span class="text-xs text-secondary ml-2">— Contextual alert box: info, warning, critical, success</span></div>
        <div class="space-y-2">
          <Admonition type="info" header="Info" body="An informational message." />
          <Admonition type="warning" header="Warning" body="Something to watch out for." />
          <Admonition type="success" header="Success" body="Operation completed." />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">StackedAdmonitions</code><span class="text-xs text-secondary ml-2">— Animated stack of admonitions with expand/collapse</span></div>
        <StackedAdmonitions :items="[{ id: 'update', type: 'info' }, { id: 'compat', type: 'warning' }]">
          <template #item="{ item }">
            <Admonition v-if="item.id === 'update'" type="info" header="Update available" body="A new version is ready to install." />
            <Admonition v-else type="warning" header="Compatibility issue" body="This version requires Fabric 0.16+." />
          </template>
        </StackedAdmonitions>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ServerNotice</code><span class="text-xs text-secondary ml-2">— Server-wide notice banner (info, warn, critical)</span></div>
        <div class="space-y-2">
          <ServerNotice level="info" message="Scheduled maintenance on Sunday at 2:00 AM UTC." :dismissable="true" />
          <ServerNotice level="warn" title="Warning" message="High server load detected." :dismissable="false" />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ErrorInformationCard</code><span class="text-xs text-secondary ml-2">— Error display card with title and details</span></div>
        <ErrorInformationCard title="Something went wrong" description="An unexpected error occurred. Please try again." />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">EmptyState</code><span class="text-xs text-secondary ml-2">— Illustrated empty state with heading and description</span></div>
        <EmptyState type="empty-inbox" heading="No content yet" description="Add something to get started." />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">UnsavedChangesPopup</code><span class="text-xs text-secondary ml-2">— Sticky popup shown when there are unsaved changes</span></div>
        <div class="relative h-20">
          <UnsavedChangesPopup :original="{ name: 'Original Name' }" :modified="{ name: 'Modified Name' }" @save="() => {}" @reset="() => {}" />
        </div>
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  7. LOADING & PROGRESS                    -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Loading &amp; Progress</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">LoadingBar</code><span class="text-xs text-secondary ml-2">— Indeterminate linear loading indicator</span></div>
        <LoadingBar :loading="true" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">LoadingIndicator</code><span class="text-xs text-secondary ml-2">— Animated spinning loading indicator</span></div>
        <LoadingIndicator />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProgressBar</code><span class="text-xs text-secondary ml-2">— Linear progress indicator with color variants</span></div>
        <div class="space-y-2">
          <ProgressBar :progress="0.65" color="brand" label="Uploading..." :show-progress="true" />
          <ProgressBar :progress="0.9" color="green" label="Verifying" :show-progress="true" />
          <ProgressBar :progress="0.4" color="orange" striped label="Queued" />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProgressSpinner</code><span class="text-xs text-secondary ml-2">— Circular progress spinner with percentage</span></div>
        <div class="flex gap-4 items-center">
          <ProgressSpinner :progress="0.35" />
          <ProgressSpinner :progress="0.7" />
          <ProgressSpinner :progress="1" />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">AppearingProgressBar</code><span class="text-xs text-secondary ml-2">— Progress bar that fades/grows into view</span></div>
        <AppearingProgressBar :max-value="100" :current-value="65" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ReadyTransition</code><span class="text-xs text-secondary ml-2">— Fade/slide transition for async-loaded content</span></div>
        <ReadyTransition :ready="true">
          <div class="p-3 bg-bg-raised rounded text-secondary text-sm">Content faded in when ready.</div>
        </ReadyTransition>
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  8. NAVIGATION                            -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Navigation</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">NavTabs</code><span class="text-xs text-secondary ml-2">— Navigation tab bar with active state</span></div>
        <NavTabs mode="local" :active-index="navTabIndex" :links="[{ label: 'Overview', href: '' }, { label: 'Gallery', href: 'gallery' }, { label: 'Changelog', href: 'changelog' }, { label: 'Versions', href: 'versions' }]" @tab-click="navTabIndex = $event" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Pagination</code><span class="text-xs text-secondary ml-2">— Page navigation controls</span></div>
        <Pagination :page="4" :count="12" />
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  9. CONTENT & DATA                        -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Content &amp; Data</h2>
    <div class="space-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ContentPageHeader</code><span class="text-xs text-secondary ml-2">— Full page header with icon, title, stats, and actions</span></div>
        <ContentPageHeader>
          <template #icon><Avatar src="https://cdn.modrinth.com/data/AANobbMI/icon.png" size="64px" /></template>
          <template #title>Sodium</template>
          <template #summary>A modern rendering engine for Minecraft.</template>
          <template #stats><span>30M downloads</span><span>45K followers</span></template>
          <template #actions><ButtonStyled color="brand"><button>Follow</button></ButtonStyled></template>
        </ContentPageHeader>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProjectCard</code><span class="text-xs text-secondary ml-2">— Rich project listing card with grid and list layout modes</span></div>
        <div class="display-mode--grid">
          <ProjectCard v-bind="projectCardData" />
        </div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">Table</code><span class="text-xs text-secondary ml-2">— Data table with sorting, row selection, and custom cell slots</span></div>
        <Table :columns="tableColumns" :data="tableData" show-selection row-key="name" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">MarkdownEditor</code><span class="text-xs text-secondary ml-2">— Rich markdown text editor with preview mode</span></div>
        <div class="h-64">
          <MarkdownEditor v-model="markdownValue" placeholder="Write markdown..." />
        </div>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

        <div class="space-y-2">
          <div><code class="text-sm font-bold text-brand font-mono">CopyCode</code><span class="text-xs text-secondary ml-2">— Code snippet with a one-click copy button</span></div>
          <CopyCode text="npm install @modrinth/ui" />
        </div>

        <div class="space-y-2">
          <div><code class="text-sm font-bold text-brand font-mono">Timeline</code><span class="text-xs text-secondary ml-2">— Vertical timeline for changelog or event history</span></div>
          <Timeline>
            <div class="flex gap-2 items-start text-sm"><span class="w-2 h-2 rounded-full bg-brand shrink-0 mt-1"></span><div><span class="font-semibold text-contrast">v3.0.0</span> <span class="text-secondary">— Major rewrite</span></div></div>
            <div class="flex gap-2 items-start text-sm"><span class="w-2 h-2 rounded-full bg-secondary shrink-0 mt-1"></span><div><span class="font-semibold text-contrast">v2.5.0</span> <span class="text-secondary">— Performance improvements</span></div></div>
            <div class="flex gap-2 items-start text-sm"><span class="w-2 h-2 rounded-full bg-secondary shrink-0 mt-1"></span><div><span class="font-semibold text-contrast">v1.0.0</span> <span class="text-secondary">— Initial release</span></div></div>
          </Timeline>
        </div>

      </div>
    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  10. MODALS                               -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Modals</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">NewModal</code><span class="text-xs text-secondary ml-2">— Standard dialog modal with header, body, and action slots</span></div>
        <ButtonStyled color="brand"><button @click="newModalRef?.show()">Open NewModal</button></ButtonStyled>
        <NewModal ref="newModalRef" header="Example Modal">
          <p>This is modal content. Click outside or the X to close.</p>
          <template #actions>
            <div class="flex gap-2 justify-end">
              <ButtonStyled><button @click="newModalRef?.hide()">Cancel</button></ButtonStyled>
              <ButtonStyled color="brand"><button @click="newModalRef?.hide()">Confirm</button></ButtonStyled>
            </div>
          </template>
        </NewModal>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">TabbedModal</code><span class="text-xs text-secondary ml-2">— Modal with built-in tab navigation</span></div>
        <ButtonStyled color="brand"><button @click="tabbedModalRef?.show()">Open TabbedModal</button></ButtonStyled>
        <TabbedModal ref="tabbedModalRef" header="Settings" :tabs="tabbedModalTabs" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ShareModal</code><span class="text-xs text-secondary ml-2">— Share modal with copyable link</span></div>
        <ButtonStyled color="brand"><button @click="shareModalRef?.show()">Open ShareModal</button></ButtonStyled>
        <ShareModal ref="shareModalRef" :url="'https://modrinth.com/mod/sodium'" title="Share Sodium" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ConfirmLeaveModal</code><span class="text-xs text-secondary ml-2">— Confirm dialog for navigating away with unsaved changes</span></div>
        <ButtonStyled color="orange"><button @click="confirmLeaveModalRef?.show()">Open ConfirmLeaveModal</button></ButtonStyled>
        <ConfirmLeaveModal ref="confirmLeaveModalRef" @confirm="() => {}" @cancel="() => {}" />
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">UploadProgressModal</code><span class="text-xs text-secondary ml-2">— Non-dismissable modal showing file upload progress</span></div>
        <ButtonStyled color="brand"><button @click="uploadModalRef?.show()">Open UploadProgressModal</button></ButtonStyled>
        <UploadProgressModal ref="uploadModalRef" :progress="0.6" title="Uploading version files..." />
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  11. PROJECT SIDEBAR                      -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Project Sidebar</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProjectSidebarDetails</code><span class="text-xs text-secondary ml-2">— Project dates and license info</span></div>
        <div style="max-width:300px"><ProjectSidebarDetails :project="sidebarProject" link-target="_blank" :has-versions="true" /></div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProjectSidebarCreators</code><span class="text-xs text-secondary ml-2">— Project team members with roles and avatar links</span></div>
        <div style="max-width:300px"><ProjectSidebarCreators :members="sidebarMembers" :org-link="(s) => '/org/' + s" :user-link="(u) => '/user/' + u" link-target="_blank" /></div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProjectSidebarLinks</code><span class="text-xs text-secondary ml-2">— Project external links (source, issues, wiki, donations)</span></div>
        <div style="max-width:300px"><ProjectSidebarLinks :project="sidebarLinks" link-target="_blank" /></div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProjectSidebarTags</code><span class="text-xs text-secondary ml-2">— Project category and loader tags in the sidebar</span></div>
        <div style="max-width:300px"><ProjectSidebarTags :categories="[{ name: 'optimization' }, { name: 'lightweight' }, { name: 'fabric' }]" /></div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProjectSidebarCompatibility</code><span class="text-xs text-secondary ml-2">— Compatible loaders and game versions in the sidebar</span></div>
        <div style="max-width:300px"><ProjectSidebarCompatibility :project="compatibilityProject" :tags="compatibilityTags" /></div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProjectSidebarServerInfo</code><span class="text-xs text-secondary ml-2">— Minecraft server connection info in the project sidebar</span></div>
        <div style="max-width:300px"><ProjectSidebarServerInfo :project-v3="serverInfoProject" :tags="compatibilityTags" :ping="42" /></div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">ProjectCombobox</code><span class="text-xs text-secondary ml-2">— Searchable project selector querying the Modrinth API</span></div>
        <ProjectCombobox placeholder="Search for a project..." />
      </div>

    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  12. SERVERS                              -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Servers</h2>

    <div class="space-y-2">
      <div><code class="text-sm font-bold text-brand font-mono">ServerListing</code><span class="text-xs text-secondary ml-2">— Server card with status, game version, loader, and IP</span></div>
      <div style="max-width:920px"><ServerListing v-bind="serverData" /></div>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">
      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">InstallingBanner</code><span class="text-xs text-secondary ml-2">— Banner shown while a server is being configured</span></div>
        <InstallingBanner />
      </div>
      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">SaveBanner</code><span class="text-xs text-secondary ml-2">— Sticky save/discard banner for server settings changes</span></div>
        <SaveBanner :show="true" @save="() => {}" @discard="() => {}" />
      </div>
    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  13. INSTANCES                            -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-4">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Instances</h2>
    <div class="space-y-2">
      <div><code class="text-sm font-bold text-brand font-mono">ContentModpackCard</code><span class="text-xs text-secondary ml-2">— Modpack card for instance content tabs with version, owner, and actions</span></div>
      <ContentModpackCard :project="modpackProject" :version="modpackVersion" :owner="modpackOwner" @update="() => {}" @content="() => {}" />
    </div>
  </section>

  <!-- ══════════════════════════════════════════ -->
  <!--  14. SPECIALIZED                          -->
  <!-- ══════════════════════════════════════════ -->
  <section class="space-y-8">
    <h2 class="text-2xl font-bold text-contrast pb-3 border-b-2 border-brand">Specialized</h2>
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-10 gap-y-8">

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">BaseTerminal</code><span class="text-xs text-secondary ml-2">— Terminal/console output with ANSI color support</span></div>
        <div style="height:180px;width:100%"><BaseTerminal ref="termRef" /></div>
      </div>

      <div class="space-y-2">
        <div><code class="text-sm font-bold text-brand font-mono">EnvironmentIndicator</code><span class="text-xs text-secondary ml-2">— Shows where a mod runs: client, server, or both</span></div>
        <div class="flex flex-col gap-2">
          <div class="flex items-center gap-2"><EnvironmentIndicator type="mod" client-side="required" server-side="optional" /><span class="text-xs text-secondary">client required, server optional</span></div>
          <div class="flex items-center gap-2"><EnvironmentIndicator type="mod" client-side="optional" server-side="required" /><span class="text-xs text-secondary">server only</span></div>
          <div class="flex items-center gap-2"><EnvironmentIndicator type="mod" client-side="required" server-side="required" /><span class="text-xs text-secondary">client and server</span></div>
        </div>
      </div>

    </div>
  </section>

</div>
    `,
	}),
}