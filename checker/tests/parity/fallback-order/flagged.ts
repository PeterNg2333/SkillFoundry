let mode: string | null = null;
if (iframe) mode = 'kiosk';
else mode = mobile ? 'mobile' : 'desktop';
if (mode === null && phone) mode = 'mobile';
if (mode === null && tablet) mode = 'tablet';
if (layout === null && wide) width = 'wide';
