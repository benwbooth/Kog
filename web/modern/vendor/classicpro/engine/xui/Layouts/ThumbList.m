/**
 * ThumbList
 *
 * @author mpdeimos
 * @date 09/04/17
 */

#include <lib/std.mi>

Function int max (int a, int b);
Function updateThumbs();

Global Group scriptGroup;
Global GuiObject coContent;
Global Group content;
Global int spacing;

System.onScriptLoaded ()
{
	scriptGroup = getScriptGroup();
	coContent = scriptGroup.getObject("content");
	content = null;
	spacing = 0;
}

System.onSetXuiParam (String param, String value)
{
	if (strlower(param) == "content")
	{
		coContent.setXmlParam("groupid", value);
		if (value != "")
		{
			coContent.show();
			content = coContent.findObject(value);
			updateThumbs();
		}
		else
		{
			coContent.hide();
			content = null;
		}
	}
	else if (strlower(param) == "spacing")
	{
		spacing = stringToInteger(value);		
	}
}

scriptGroup.onAction (String action, String param, Int x, int y, int p1, int p2, GuiObject source)
{
	if (action == "update")
	{
		updateThumbs();
	}
}

scriptGroup.onResize (int x, int y, int w, int h)
{
	updateThumbs();
}

updateThumbs()
{
	if (content == null || !scriptGroup.isVisible())
	{
		return;
	}

	int w = scriptGroup.getWidth();
	int h = scriptGroup.getHeight();
	
	int lineHeight = 0;
	int yOffset = 0;
	int xOffset = 0;

	for ( int i = 0; i < content.getNumObjects(); i++ )
	{
		GuiObject go = content.enumObject(i);

		if (go.isVisible())
		{
			if (strupper(go.getID()) == "NL")
			{
				yOffset += lineHeight+spacing;
				xOffset = 0;
			}
			else if (!xOffset || xOffset + go.getWidth() <= w)
			{
				go.setXmlParam("x", integerToString(xOffset));
				go.setXmlParam("y", integerToString(yOffset));

		
				xOffset += spacing + go.getWidth();
				lineHeight = max(lineHeight, go.getHeight());
			}
			else
			{
				go.setXmlParam("x", "0");
				yOffset += lineHeight+spacing;
				go.setXmlParam("y", integerToString(yOffset));

				xOffset = spacing + go.getWidth();
				lineHeight = go.getHeight();
			}
		}
	}
}

scriptGroup.onSetVisible (Boolean onoff)
{
	if (onoff)
	{
		updateThumbs();
	}
}

int max (int a, int b)
{
	if (a > b)
	{
		return a;
	}

	return b;
}	