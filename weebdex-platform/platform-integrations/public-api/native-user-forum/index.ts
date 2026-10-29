// Only native-user profile routes use this read-only discussion surface.
// Other native/public discussion routes keep their existing adapters.
export {default as Provider} from '../native-forum/Provider.svelte';
export {default as Comments} from './Comments.svelte';

// Explicit advanced tooling only; ordinary routes always use Comments.
export {default as AdvancedComments} from './AdvancedComments.svelte';
