/**
 * Handles Widget Manager Items
 *
 * @author mpdeimos
 * @date 2009/04/28
 */

#include <lib/std.mi>
#include "../lib/ClassicProFile.mi"

Global Text name, author, version;
Global Layer thumbnail;
Global String thumbnailImg, uninstallerPath, widgetID, supportURL;
Global Button uninstaller, support;
Global Group sg;

Global Button showmini, showdrawer, showmain;
Global String miniX, drawerX, mainX;

System.onScriptLoaded ()
{
	sg = getScriptGroup();

	author = sg.getObject("author");
	version = sg.getObject("version");
	name = sg.getObject("name");

	thumbnail = sg.getObject("thumbnail");
	thumbnailImg = thumbnail.getXmlParam("image");
	
	uninstaller = sg.getObject("uninstall");
	support = sg.getObject("support");
	
	showmini = sg.getObject("show.mini");
	showdrawer = sg.getObject("show.drawer");
	showmain = sg.getObject("show.main");

	uninstallerPath = "";
}

system.onSetXuiParam (String param, String value)
{
	if (strlower(param) == "widgetname")
	{
		name.setText(value);
		version.setXmlParam("x", integerToString(66 + name.getTextWidth()));
	}
	else if (strlower(param) == "hilight")
	{
		sg.getObjecT("f1").hide();
		sg.getObjecT("f2").show();
		name.setXmlParam("x", "88");
		version.setXmlParam("x", integerToString(88 + name.getTextWidth()));
	}
	else if (strlower(param) == "widgetpos_mini"){
		showmini.setXmlParam("ghost", "0");
		showmini.setAlpha(255);
		miniX = value;
		//debug(strlower(value));
	}
	else if (strlower(param) == "widgetpos_drawer"){
		showdrawer.setXmlParam("ghost", "0");
		showdrawer.setAlpha(255);
		drawerX = value;
	}
	else if (strlower(param) == "widgetpos_main"){
		showmain.setXmlParam("ghost", "0");
		showmain.setAlpha(255);
		mainX = value;
	}
	else if (strlower(param) == "widgetauthor")
	{
		author.setText("by "+value);
	}
	else if (strlower(param) == "widgetversion")
	{
		version.setText(value);
	}
	else if (strlower(param) == "widgetid")
	{
		widgetID = value;
		thumbnail.setXmlParam("image", value+".thumbnail");
		if (thumbnail.isInvalid())
		{
			thumbnail.setXmlParam("image", thumbnailImg);
		}
		
	}
	else if (strlower(param) == "widgetuninstaller")
	{
		uninstaller.setXmlParam("ghost", "0");
		uninstaller.setAlpha(255);
		uninstallerPath = value;
	}
	else if (strlower(param) == "widgetsupport")
	{
		support.setXmlParam("ghost", "0");
		support.setAlpha(255);
		supportURL = value;
	}
}

uninstaller.onLeftClick ()
{
	ClassicProFile.openFile(uninstallerPath, "");
}
support.onLeftClick ()
{
	System.navigateUrl(supportURL);
	//debug(widgetID+" _ " + sg.getXmlParam("userdata"));
	//getParentLayout().sendAction("show_widget", widgetID,stringToInteger(sg.getXmlParam("userdata")),0,0,0);
}

/*
showmain.onLeftClick ()
{
	//debug(sg.getXmlParam("userdata"));
	getParentLayout().sendAction("show_widget", widgetID,stringToInteger(sg.getXmlParam("userdata")),0,0,0);
}*/

showmain.onLeftClick ()
{
	getParentLayout().sendAction("show_widget", widgetID,stringToInteger(mainX),0,0,0);
}

showdrawer.onLeftClick ()
{
	getParentLayout().sendAction("show_widget", widgetID,stringToInteger(drawerX),0,0,0);
}

showmini.onLeftClick ()
{
	getParentLayout().sendAction("show_widget", widgetID,stringToInteger(miniX),0,0,0);
}

