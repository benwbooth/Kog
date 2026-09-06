/*---------------------------------------------------
BASED ON BENTO SCRIPTS BY martin.deimos ;)
Depending Files:
		scripts/maximize.m
---------------------------------------------------*/

#ifndef included
#error This script can only be compiled as a #include
#endif

#include "../../../scripts/attribs/gen_pageguids.m"


Function initAttribs_Autoresize();


#define CUSTOM_PAGE_RESIZE "{E704AB5A-108E-4309-B54B-43EBA5C0C3AA}"

Global ConfigAttribute titlebar_dblclk_max_attib, titlebar_dblclk_shade_attib, collapse_top_attrib, collapse_bottom_attrib, linkPosWidth;

initAttribs_Autoresize(){
	initPages();
	
	ConfigItem custom_page_autoresize = addConfigSubMenu(optionsmenu_page, "Window Sizing", CUSTOM_PAGE_RESIZE);

	titlebar_dblclk_max_attib = custom_page_autoresize.newAttribute("Maximize Window on Titlebar Doubleclick", "1");
	titlebar_dblclk_shade_attib = custom_page_autoresize.newAttribute("Switch to Shade on Titlebar Doubleclick", "0");
	addMenuSeparator(custom_page_autoresize);
	collapse_top_attrib = custom_page_autoresize.newAttribute("Collapse Window to Top", "1");
	collapse_bottom_attrib = custom_page_autoresize.newAttribute("Collapse Window to Bottom", "0");
	addMenuSeparator(custom_page_autoresize);
	linkPosWidth = custom_page_autoresize.newAttribute("Link Shade-mode Position and Width", "0");
}

#ifdef MAIN_ATTRIBS_MGR

titlebar_dblclk_shade_attib.onDataChanged()
{
	if (attribs_mychange) return;
	NOOFF
	attribs_mychange = 1;
	if (getData() == "1") titlebar_dblclk_max_attib.setData("0");
	if (getData() == "0") titlebar_dblclk_max_attib.setData("1");
	attribs_mychange = 0;
}
titlebar_dblclk_max_attib.onDataChanged()
{
	if (attribs_mychange) return;
	NOOFF
	attribs_mychange = 1;
	if (getData() == "1") titlebar_dblclk_shade_attib.setData("0");
	if (getData() == "0") titlebar_dblclk_shade_attib.setData("1");
	attribs_mychange = 0;
}

collapse_top_attrib.onDataChanged()
{
	if (attribs_mychange) return;
	NOOFF
	attribs_mychange = 1;
	if (getData() == "1") collapse_bottom_attrib.setData("0");
	if (getData() == "0") collapse_bottom_attrib.setData("1");
	attribs_mychange = 0;
}
collapse_bottom_attrib.onDataChanged()
{
	if (attribs_mychange) return;
	NOOFF
	attribs_mychange = 1;
	if (getData() == "1") collapse_top_attrib.setData("0");
	if (getData() == "0") collapse_top_attrib.setData("1");
	attribs_mychange = 0;
}
linkPosWidth.onDataChanged()
{
	if (attribs_mychange) return;
	NOOFF
	attribs_mychange = 1;
	if (getData() == "1") linkPosWidth.setData("0");
	if (getData() == "0") linkPosWidth.setData("1");
	attribs_mychange = 0;
}
#endif