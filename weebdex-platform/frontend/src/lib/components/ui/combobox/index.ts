import GroupHeading from "$lib/components/ui/select/select-group-heading.svelte";
import Group from "$lib/components/ui/select/select-group.svelte";
import Label from "$lib/components/ui/select/select-label.svelte";
import Portal from "$lib/components/ui/select/select-portal.svelte";
import ScrollDownButton from "$lib/components/ui/select/select-scroll-down-button.svelte";
import ScrollUpButton from "$lib/components/ui/select/select-scroll-up-button.svelte";
import Separator from "$lib/components/ui/select/select-separator.svelte";
import Trigger from "$lib/components/ui/select/select-trigger.svelte";
import Root from "$lib/components/ui/select/select.svelte";
import Content from "./combobox-content.svelte";
import Item from "./combobox-item.svelte";

export {
	Content,
	Group,
	GroupHeading,
	Item,
	Label,
	Portal,
	Root,
	ScrollDownButton,
	ScrollUpButton,
	//
	Root as Select,
	Content as SelectContent,
	Group as SelectGroup,
	GroupHeading as SelectGroupHeading,
	Item as SelectItem,
	Label as SelectLabel,
	Portal as SelectPortal,
	ScrollDownButton as SelectScrollDownButton,
	ScrollUpButton as SelectScrollUpButton,
	Separator as SelectSeparator,
	Trigger as SelectTrigger,
	Separator,
	Trigger
};
