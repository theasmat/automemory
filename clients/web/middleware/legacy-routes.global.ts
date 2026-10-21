// Pages that were merged into others keep working from old bookmarks and links.
const MOVED: Record<string, string> = {
  '/overview': '/',
  '/memories': '/brain',
  '/coordination': '/agents',
  '/proposals': '/inbox',
  '/guardrails': '/safety',
  '/recall': '/models?tab=search',
  '/skills': '/activity?tab=skills',
  '/traces': '/activity?tab=traces',
  '/failures': '/activity?tab=problems',
  '/replay': '/activity?tab=sessions',
};

export default defineNuxtRouteMiddleware((to) => {
  const target = MOVED[to.path];
  if (target) return navigateTo(target, { replace: true });
});
