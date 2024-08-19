import React from 'react'
import Link from 'next/link'

export const links = [
  { href: '/', tag: 'pre-generated' },
  { href: '/metadata', tag: 'pre-generated' },
  { href: '/nested/a', tag: 'pre-generated' },
  { href: '/nested/b', tag: 'on-demand' },
  { href: '/nested/c', tag: 'on-demand' },
  { href: '/on-demand/a', tag: 'on-demand, no-gsp' },
  { href: '/on-demand/b', tag: 'on-demand, no-gsp' },
  { href: '/on-demand/c', tag: 'on-demand, no-gsp' },
  { href: '/loading/a', tag: 'loading.jsx, pre-generated' },
  { href: '/loading/b', tag: 'loading.jsx, on-demand' },
  { href: '/loading/c', tag: 'loading.jsx, on-demand' },
  { href: '/static', tag: 'static' },
  { href: '/no-suspense', tag: 'no suspense' },
  { href: '/no-suspense/nested/a', tag: 'no suspense, pre-generated' },
  { href: '/no-suspense/nested/b', tag: 'no suspense, on-demand' },
  { href: '/no-suspense/nested/c', tag: 'no suspense, on-demand' },
  { href: '/dynamic/force-dynamic', tag: "dynamic = 'force-dynamic'" },
  {
    href: '/dynamic/force-dynamic/nested/a',
    tag: "dynamic = 'force-dynamic', on-demand, no-gsp",
  },
  {
    href: '/dynamic/force-dynamic/nested/b',
    tag: "dynamic = 'force-dynamic', on-demand, no-gsp",
  },
  {
    href: '/dynamic/force-dynamic/nested/c',
    tag: "dynamic = 'force-dynamic', on-demand, no-gsp",
  },
  { href: '/dynamic/force-static', tag: "dynamic = 'force-static'" },
  { href: '/edge/suspense', tag: 'edge, pre-generated' },
  { href: '/edge/suspense/a', tag: 'edge, pre-generated' },
  { href: '/edge/suspense/b', tag: 'edge, on-demand' },
  { href: '/edge/suspense/c', tag: 'edge, on-demand' },
  { href: '/edge/no-suspense', tag: 'edge, no suspense, pre-generated' },
  { href: '/edge/no-suspense/a', tag: 'edge, no suspense, pre-generated' },
  { href: '/edge/no-suspense/b', tag: 'edge, no suspense, on-demand' },
  { href: '/edge/no-suspense/c', tag: 'edge, no suspense, on-demand' },
  { href: '/navigation/not-found', tag: 'not-found' },
  { href: '/navigation/not-found/no-store', tag: 'not-found, dynamic' },
  { href: '/navigation/redirect', tag: 'redirect' },
  { href: '/navigation/redirect/no-store', tag: 'redirect, dynamic' },
  { href: '/pages', tag: 'pages' },
]

export const Links = () => {
  return (
    <ul>
      {links.map(({ href, tag }) => (
        <li key={href}>
          <Link href={href}>{href}</Link> <span>{tag}</span>
        </li>
      ))}
    </ul>
  )
}
