/**
 * Handles Widget Manager Items
 *
 * @author mpdeimos
 * @date 2009/04/28
 */

#include <lib/std.mi>
#include "../lib/ClassicProFile.mi"

Global Text name, author, version, position;
Global Layer thumbnail;
Global String thumbnailImg, uninstallerPath, widgetID, supportURL;
Global Button uninstaller, showWidget, support;
Global GuiObject uninstaller1, uninstaller2, support1, support2;
Global Group sg;

System.onScriptLoaded ()
{
	sg = getScriptGroup();

	author = sg.getObject("author");
	version = sg.getObject("version");
	name = sg.getObject("name");
	position = sg.getObject("position");
	thumbnail = sg.getObject("thumbnail");
	thumbnailImg = thumbnail.getXmlParam("image");
	uninstaller = sg.getObject("uninstall");
	uninstaller1 = sg.getObject("uninstall.1");
	uninstaller2 = sg.getObject("uninstall.2");
	support = sg.getObject("support");
	support1 = sg.getObject("support.1");
	support2 = sg.getObject("support.2");
	showWidget = sg.getObject("show");
	uninstallerPath = "";
}

system.onSetXuiParam (String param, String value)
{
	if (strlower(param) == "widgetname")
	{
		name.setText(value);
	}
	else if (strlower(param) == "widgetposition")
	{
		position.setText(value);
	}
	else if (strlower(param) == "widgetauthor")
	{
		author.setText(value);
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
		if (value != "")
		{
			uninstallerPath = value;
			uninstaller.show();
			uninstaller1.show();
			uninstaller2.show();
		}
		else
		{
			uninstaller.hide();
			uninstaller1.hide();
			uninstaller2.hide();
		}
	}
	else if (strlower(param) == "widgetsupport")
	{
		if (value != "")
		{
			supportURL = value;
			support.show();
			support1.show();
			support2.show();
		}
		else
		{
			support.hide();
			support1.hide();
			support2.hide();
		}
		
	}
}

uninstaller.onLeftClick ()
{
	ClassicProFile.openFile(uninstallerPath, "");
}

showWidget.onLeftClick ()
{
	getParentLayout().sendAction("show_widget", widgetID,stringToInteger(sg.getXmlParam("userdata")),0,0,0);
}

support.onLeftClick ()
{
	System.navigateUrl(supportURL);
}
