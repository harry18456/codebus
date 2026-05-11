// Tiny Lucide-flavored icon set inlined as SVGs.
// 1.5 stroke, 14px default — matches shadcn/ui's Lucide config.
const cbIcon = (paths, { size = 14, stroke = 1.5 } = {}) => ({ size: s = size, ...rest } = {}) => (
  <svg
    width={s} height={s} viewBox="0 0 24 24" fill="none"
    stroke="currentColor" strokeWidth={stroke}
    strokeLinecap="round" strokeLinejoin="round"
    {...rest}
  >
    {paths}
  </svg>
);

const IconPlus     = cbIcon(<><path d="M12 5v14M5 12h14" /></>);
const IconSearch   = cbIcon(<><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></>);
const IconSettings = cbIcon(<><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 0 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z" /></>);
const IconKebab    = cbIcon(<><circle cx="12" cy="5" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="12" cy="19" r="1"/></>);
const IconChevR    = cbIcon(<><path d="m9 18 6-6-6-6"/></>);
const IconCommand  = cbIcon(<><path d="M18 3a3 3 0 0 0-3 3v12a3 3 0 0 0 3 3 3 3 0 0 0 3-3 3 3 0 0 0-3-3H6a3 3 0 0 0-3 3 3 3 0 0 0 3 3 3 3 0 0 0 3-3V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3 3 3 0 0 0 3 3h12a3 3 0 0 0 3-3 3 3 0 0 0-3-3Z" /></>);
const IconRefresh  = cbIcon(<><path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/></>);
const IconFilter   = cbIcon(<><path d="M3 6h18M7 12h10M10 18h4"/></>);
const IconBell     = cbIcon(<><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10 21a2 2 0 0 0 4 0"/></>);
const IconFolder   = cbIcon(<><path d="M4 5a2 2 0 0 1 2-2h3l2 2h7a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2Z"/></>);
const IconStop     = cbIcon(<><rect x="6" y="6" width="12" height="12" rx="1"/></>);
const IconPlay     = cbIcon(<><path d="m7 5 12 7-12 7Z"/></>);
const IconCheck    = cbIcon(<><path d="m5 12 5 5 9-11"/></>);

Object.assign(window, {
  IconPlus, IconSearch, IconSettings, IconKebab, IconChevR, IconCommand,
  IconRefresh, IconFilter, IconBell, IconFolder, IconStop, IconPlay, IconCheck,
});
