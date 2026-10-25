// Only native-user profile routes use this read-only discussion surface.
// Other native/public discussion routes keep their existing adapters.
export {default as Provider} from '../native-forum/Provider.svelte';
export {default as Comments} from './Comments.svelte';
