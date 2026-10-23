import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://mixum-agent-protocol.dev',
  integrations: [
    starlight({
      title: 'Mixum Agent Protocol',
      description:
        'An open protocol connecting AI agents to isolated, snapshot-capable environments across macOS, Windows, Linux, local and cloud.',
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/EmergeShift/Mixum-Agent-Protocol',
        },
      ],
      locales: {
        root: { label: 'English', lang: 'en' },
        zh: { label: '简体中文', lang: 'zh-CN' },
      },
      sidebar: [
        {
          label: 'Introduction',
          items: [{ label: 'Introduction', link: '/' }, 'quickstart'],
        },
        {
          label: 'Specification',
          items: [
            'spec/overview',
            'spec/architecture',
            'spec/transport-security',
            'spec/envelope',
            'spec/session',
            'spec/tasks',
            'spec/agents-bridge',
            'spec/snapshots',
            'spec/mobility',
            'spec/display',
            'spec/privilege',
            'spec/memory',
            'spec/vm-host',
            'spec/versioning-governance',
          ],
        },
        {
          label: 'Artifacts',
          items: ['artifacts', 'sdk'],
        },
      ],
    }),
  ],
});
