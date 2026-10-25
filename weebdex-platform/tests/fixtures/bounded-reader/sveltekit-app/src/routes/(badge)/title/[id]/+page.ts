import { fixtureRouteData } from '$badgeFixture/fixture-route-data';
export const load = (input: Parameters<typeof fixtureRouteData>[0]) => fixtureRouteData(input, 'title');
