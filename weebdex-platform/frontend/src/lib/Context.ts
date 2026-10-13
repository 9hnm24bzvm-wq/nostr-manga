import type { Mutex } from "$lib";
import { createContext } from "svelte";
import type { Writable } from "svelte/store";

export const [getMutex, setMutex] = createContext<Mutex>();
export const [getUser, setUser] = createContext<Writable<User>>();
export const [getManga, setManga] = createContext<() => Manga>();
export const [getAuthor, setAuthor] = createContext<Writable<Author>>();
export const [getChapter, setChapter] = createContext<() => Chapter>();
export const [getChaptersCount, setChaptersCount] = createContext<() => number | undefined>();
export const [getGroup, setGroup] = createContext<Writable<ScanlationGroup>>();
export const [getUploadSession, setUploadSession] = createContext<() => UploadSession | undefined>();

export const [getReadHistory, setReadHistory] =
	createContext<Writable<Record<string, Record<string, null>> | undefined>>();
export const [getKey, setKey] = createContext<Writable<number>>();
