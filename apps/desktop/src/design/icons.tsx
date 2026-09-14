import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

function Icon({ children, ...props }: IconProps & { children: React.ReactNode }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      width="16"
      height="16"
      aria-hidden="true"
      {...props}
    >
      {children}
    </svg>
  );
}

export const IconDoc = (p: IconProps) => (
  <Icon {...p}>
    <path d="M7 3h7l4 4v14H7z" />
    <path d="M10 12h6M10 15.5h6M10 8.5h3" />
  </Icon>
);

export const IconSheet = (p: IconProps) => (
  <Icon {...p}>
    <rect x="3.5" y="4.5" width="17" height="15" rx="1.5" />
    <path d="M3.5 9.5h17M3.5 14.5h17M9.5 4.5v15M15 4.5v15" />
  </Icon>
);

export const IconPdf = (p: IconProps) => (
  <Icon {...p}>
    <path d="M7 3h7l4 4v14H7z" />
    <path d="M14 3v4h4" />
    <path d="M9.5 17v-5h1.6a1.6 1.6 0 0 1 0 3.2H9.5" />
  </Icon>
);

export const IconImage = (p: IconProps) => (
  <Icon {...p}>
    <rect x="3.5" y="5" width="17" height="14" rx="2" />
    <circle cx="8.8" cy="10" r="1.4" />
    <path d="M4 16.5l4.6-4 3.4 3 3-2.4 5 4.1" />
  </Icon>
);

export const IconConvert = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 8h12l-3-3M20 16H8l3 3" />
  </Icon>
);

export const IconPlus = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 5v14M5 12h14" />
  </Icon>
);

export const IconClose = (p: IconProps) => (
  <Icon {...p}>
    <path d="M6 6l12 12M18 6L6 18" />
  </Icon>
);

export const IconSearch = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="11" cy="11" r="6" />
    <path d="M15.5 15.5L20 20" />
  </Icon>
);

export const IconCommand = (p: IconProps) => (
  <Icon {...p}>
    <path d="M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z" />
  </Icon>
);

export const IconSun = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M19.1 4.9l-1.4 1.4M6.3 17.7l-1.4 1.4" />
  </Icon>
);

export const IconMoon = (p: IconProps) => (
  <Icon {...p}>
    <path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z" />
  </Icon>
);

export const IconSettings = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.9 1.2v.2a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-2.9-1.2l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0-1.2-2.9H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.2-2.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 2.9-1.2V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 2.9 1.2l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0 1.2 2.9h.2a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.6 1z" />
  </Icon>
);

export const IconSave = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 4h11l3 3v13H5z" />
    <path d="M8.5 4v5h7V4M8.5 20v-6h7v6" />
  </Icon>
);

export const IconFolderOpen = (p: IconProps) => (
  <Icon {...p}>
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v1" />
    <path d="M3 9h17l-2.2 9.2a2 2 0 0 1-1.9 1.3H5a2 2 0 0 1-2-2z" />
  </Icon>
);

export const IconUndo = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 9h10a5 5 0 0 1 0 10H9" />
    <path d="M8 5L4 9l4 4" />
  </Icon>
);

export const IconRedo = (p: IconProps) => (
  <Icon {...p}>
    <path d="M20 9H10a5 5 0 0 0 0 10h5" />
    <path d="M16 5l4 4-4 4" />
  </Icon>
);

export const IconBold = (p: IconProps) => (
  <Icon {...p}>
    <path d="M7 5h6a3.5 3.5 0 0 1 0 7H7zM7 12h7a3.5 3.5 0 0 1 0 7H7z" />
  </Icon>
);

export const IconItalic = (p: IconProps) => (
  <Icon {...p}>
    <path d="M10 5h8M6 19h8M14 5l-4 14" />
  </Icon>
);

export const IconUnderline = (p: IconProps) => (
  <Icon {...p}>
    <path d="M6 4v6a6 6 0 0 0 12 0V4M5 20h14" />
  </Icon>
);

export const IconStrike = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 12h14M8 7a3.5 3.5 0 0 1 3.5-2.5h2A3 3 0 0 1 16 7M7 16.5A3.5 3.5 0 0 0 10.5 19h2.6a3.2 3.2 0 0 0 3-3" />
  </Icon>
);

export const IconAlignLeft = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 6h16M4 12h10M4 18h13" />
  </Icon>
);

export const IconAlignCenter = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 6h16M7 12h10M5.5 18h13" />
  </Icon>
);

export const IconAlignRight = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 6h16M10 12h10M7 18h13" />
  </Icon>
);

export const IconAlignJustify = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 6h16M4 12h16M4 18h16" />
  </Icon>
);

export const IconListBullet = (p: IconProps) => (
  <Icon {...p}>
    <path d="M9 7h11M9 12h11M9 17h11" />
    <circle cx="4.6" cy="7" r="1.1" />
    <circle cx="4.6" cy="12" r="1.1" />
    <circle cx="4.6" cy="17" r="1.1" />
  </Icon>
);

export const IconListNumber = (p: IconProps) => (
  <Icon {...p}>
    <path d="M9.5 7h11M9.5 12h11M9.5 17h11" />
    <path d="M3.6 5.6L5 5v4M3.4 11.6c.3-.5 1.6-.6 1.7.4.1.8-1.6 1.6-1.7 2.4h1.9M3.4 15.9h1.5c.9 0 .9 1.3 0 1.3h-.4c.9 0 .9 1.4 0 1.4H3.4" />
  </Icon>
);

export const IconTable = (p: IconProps) => (
  <Icon {...p}>
    <rect x="3.5" y="5" width="17" height="14" rx="1.5" />
    <path d="M3.5 10h17M9.5 5v14M15 5v14" />
  </Icon>
);

export const IconZoomIn = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="11" cy="11" r="6" />
    <path d="M11 8.5v5M8.5 11h5M15.5 15.5L20 20" />
  </Icon>
);

export const IconZoomOut = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="11" cy="11" r="6" />
    <path d="M8.5 11h5M15.5 15.5L20 20" />
  </Icon>
);

export const IconRotateRight = (p: IconProps) => (
  <Icon {...p}>
    <path d="M20 11a8 8 0 1 0-2.3 5.6" />
    <path d="M20 5v6h-6" />
  </Icon>
);

export const IconRotateLeft = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 11a8 8 0 1 1 2.3 5.6" />
    <path d="M4 5v6h6" />
  </Icon>
);

export const IconTrash = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 7h14M9.5 7V5h5v2M7 7l1 13h8l1-13" />
  </Icon>
);

export const IconCopy = (p: IconProps) => (
  <Icon {...p}>
    <rect x="9" y="9" width="11" height="11" rx="1.6" />
    <path d="M15 9V5.6A1.6 1.6 0 0 0 13.4 4H5.6A1.6 1.6 0 0 0 4 5.6v7.8A1.6 1.6 0 0 0 5.6 15H9" />
  </Icon>
);

export const IconArrowUp = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 19V5M6 11l6-6 6 6" />
  </Icon>
);

export const IconArrowDown = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 5v14M6 13l6 6 6-6" />
  </Icon>
);

export const IconArrowLeft = (p: IconProps) => (
  <Icon {...p}>
    <path d="M19 12H5M11 6l-6 6 6 6" />
  </Icon>
);

export const IconArrowRight = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 12h14M13 6l6 6-6 6" />
  </Icon>
);

export const IconHighlight = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 20h16" />
    <path d="M7.5 16.5L6 13l8.5-8.5a2.1 2.1 0 0 1 3 3L9 16l-1.5.5z" />
  </Icon>
);

export const IconPen = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 20l4.5-1 9-9a2 2 0 0 0-2.8-2.8l-9 9z" />
    <path d="M14 6.5l3.5 3.5" />
  </Icon>
);

export const IconNote = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 4h11l3 3v13H5z" />
    <path d="M9 11h6M9 15h4" />
  </Icon>
);

export const IconSquare = (p: IconProps) => (
  <Icon {...p}>
    <rect x="4.5" y="4.5" width="15" height="15" rx="1.5" />
  </Icon>
);

export const IconCircle = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="7.5" />
  </Icon>
);

export const IconLine = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 19L19 5" />
  </Icon>
);

export const IconArrowAnnot = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 19L19 5M19 5h-7M19 5v7" />
  </Icon>
);

export const IconTextTool = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 6h14M12 6v13M9 19h6" />
  </Icon>
);

export const IconEraser = (p: IconProps) => (
  <Icon {...p}>
    <path d="M8 20H4l6.5-6.5" />
    <path d="M11.5 6.5l6 6-5.5 5.5H8.5L5 14.5z" />
  </Icon>
);

export const IconMerge = (p: IconProps) => (
  <Icon {...p}>
    <path d="M7 4v7a5 5 0 0 0 5 5h5" />
    <path d="M7 20v-3" />
    <path d="M14 13l3 3-3 3" />
  </Icon>
);

export const IconSplit = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 4v7" />
    <path d="M12 11l-5 5M12 11l5 5" />
    <path d="M4 16h5v4H4zM15 16h5v4h-5z" />
  </Icon>
);

export const IconExtract = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 20V9" />
    <path d="M8 13l4-4 4 4" />
    <path d="M5 5h14" />
  </Icon>
);

export const IconLock = (p: IconProps) => (
  <Icon {...p}>
    <rect x="5" y="10.5" width="14" height="9.5" rx="1.8" />
    <path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5" />
  </Icon>
);

export const IconShield = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 3l7 3v6c0 4.4-3 7.6-7 9-4-1.4-7-4.6-7-9V6z" />
    <path d="M9 12l2 2 4-4" />
  </Icon>
);

export const IconCompress = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 4v5M12 15v5M9 7l3-3 3 3M9 17l3 3 3-3" />
    <path d="M4.5 12h15" />
  </Icon>
);

export const IconPage = (p: IconProps) => (
  <Icon {...p}>
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v4h4" />
  </Icon>
);

export const IconGrid = (p: IconProps) => (
  <Icon {...p}>
    <rect x="4" y="4" width="7" height="7" rx="1" />
    <rect x="13" y="4" width="7" height="7" rx="1" />
    <rect x="4" y="13" width="7" height="7" rx="1" />
    <rect x="13" y="13" width="7" height="7" rx="1" />
  </Icon>
);

export const IconEye = (p: IconProps) => (
  <Icon {...p}>
    <path d="M2.5 12S6 6 12 6s9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6z" />
    <circle cx="12" cy="12" r="2.6" />
  </Icon>
);

export const IconCheck = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 13l4 4L19 7" />
  </Icon>
);

export const IconInfo = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 11v5M12 8.2v.1" />
  </Icon>
);

export const IconWarn = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 4l8.5 15h-17z" />
    <path d="M12 10v4M12 16.6v.1" />
  </Icon>
);

export const IconPaste = (p: IconProps) => (
  <Icon {...p}>
    <path d="M9 4.5H7a2 2 0 0 0-2 2V19a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V6.5a2 2 0 0 0-2-2h-2" />
    <rect x="9" y="3" width="6" height="3.5" rx="1" />
  </Icon>
);

export const IconFilter = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 5h16l-6.2 7.3V19L10 17v-4.7z" />
  </Icon>
);

export const IconSort = (p: IconProps) => (
  <Icon {...p}>
    <path d="M7 4v16M4 8l3-4 3 4M17 20V4M14 16l3 4 3-4" />
  </Icon>
);

export const IconFunction = (p: IconProps) => (
  <Icon {...p}>
    <path d="M15.5 4.5h-1.8a2.2 2.2 0 0 0-2.2 2.2V20" />
    <path d="M8 11.5h6.5" />
  </Icon>
);

export const IconChart = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 20V4M4 20h16" />
    <rect x="7" y="12" width="3" height="6" />
    <rect x="12" y="8" width="3" height="10" />
    <rect x="17" y="14" width="3" height="4" />
  </Icon>
);

export const IconForm = (p: IconProps) => (
  <Icon {...p}>
    <rect x="4" y="5" width="16" height="14" rx="1.8" />
    <path d="M7.5 10h6M7.5 14h4" />
    <path d="M16 10v4" />
  </Icon>
);

export const IconCrop = (p: IconProps) => (
  <Icon {...p}>
    <path d="M6 3v13a2 2 0 0 0 2 2h13" />
    <path d="M3 6h13a2 2 0 0 1 2 2v13" />
  </Icon>
);

export const IconResize = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 10V4h6M20 14v6h-6" />
    <path d="M4 4l7 7M20 20l-7-7" />
  </Icon>
);

export const IconStar = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 4l2.5 5.2 5.5.7-4 3.9 1 5.7-5-2.8-5 2.8 1-5.7-4-3.9 5.5-.7z" />
  </Icon>
);

export const IconKeyboard = (p: IconProps) => (
  <Icon {...p}>
    <rect x="3" y="6" width="18" height="12" rx="2" />
    <path d="M7 10h.1M11 10h.1M15 10h.1M7 14h10" />
  </Icon>
);

export const IconRefresh = (p: IconProps) => (
  <Icon {...p}>
    <path d="M20 11a8 8 0 1 0-2.3 5.6" />
    <path d="M20 5v6h-6" />
  </Icon>
);

export const IconPdfText = (p: IconProps) => (
  <Icon {...p}>
    <path d="M7 3h7l4 4v14H7z" />
    <path d="M10 12h6M10 15h6" />
  </Icon>
);

export const IconSelect = (p: IconProps) => (
  <Icon {...p}>
    <path d="M5 3l6 16 2.2-6.4L19 10z" />
  </Icon>
);

export const IconHand = (p: IconProps) => (
  <Icon {...p}>
    <path d="M8 13V6.5a1.5 1.5 0 0 1 3 0V12" />
    <path d="M11 12V5.8a1.5 1.5 0 0 1 3 0V12" />
    <path d="M14 12V7.5a1.5 1.5 0 0 1 3 0V14c0 3.5-2.4 6-6 6-3 0-5-2-6-4l-1.2-2.4a1.4 1.4 0 0 1 2.4-1.4L8 14" />
  </Icon>
);

export const IconActivity = (p: IconProps) => (
  <Icon {...p}>
    <path d="M3 12h4l2.5-6 3.5 12 2.5-6h5.5" />
  </Icon>
);

export const IconSignature = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4 17c3 0 4-8 7-8s2 6 5 6 3-2 4-3" />
    <path d="M4 20h16" />
  </Icon>
);
