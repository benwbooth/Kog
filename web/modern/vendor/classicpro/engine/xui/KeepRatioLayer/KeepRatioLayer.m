/**
 * KeepRatioLayer
 *
 * @author mpdeimos
 * @date 09/04/17
 */

#include <lib/std.mi>

Function resetImage(String img);
Function repositionImage(int boundW, int boundH);

Global Group scriptGroup;

Global Layer image;
Member int image.height; 
Member int image.width;

Global int valign, align;

System.onScriptLoaded ()
{
	scriptGroup = getScriptGroup();
	image = scriptGroup.getObject("layer");

	valign = align = 0;
}

System.onSetXuiParam (String param, String value)
{
	if (strlower(param) == "align")
	{
		if (strlower(value) == "left" )
		{
			align = -1;
		}
		else if (strlower(value) == "right" )
		{
			align = 1;
		}
		else
		{
			align = 0;
		}
		return;
	}
	else if (strlower(param) == "valign")
	{
		if (strlower(value) == "top" )
		{
			valign = -1;
		}
		else if (strlower(value) == "bottom" )
		{
			valign = 1;
		}
		else
		{
			valign = 0;
		}
		return;
	}

	image.setXmlParam(param, value);
	if (strlower(param) == "image")
	{
		resetImage(value);
	}
}

resetImage(String img)
{
	Map m = new Map;
	m.loadMap(img);

	image.height = m.getHeight();
	image.width = m.getWidth();

	delete m;

	if (image.width * image.height == 0)
	{
		image.hide();
		return;
	}
	
	repositionImage(scriptGroup.getWidth(), scriptGroup.getHeight());
	image.show();
}

scriptGroup.onResize (int x, int y, int w, int h)
{
	repositionImage(w, h);
}


repositionImage(int boundW, int boundH)
{
	if (image.width * image.height == 0)
	{
		image.hide();
		return;
	}

	double aspX = boundW/image.width;
	double aspY = boundH/image.height;

	double asp = aspX;
	if (aspY < aspX)
		asp = aspY;

	int newW = image.width*asp;
	int newH = image.height*asp;

	// Align
	int offsetX = (boundW - newW)/2;
	if (align == 1) offsetX *=2;
	else if (align == -1) offsetX = 0;

	// Valign
	int offsetY = (boundH - newH)/2;
	if (valign == 1) offsetY *=2;
	else if (valign == -1) offsetY = 0;

	image.setXmlParam("x", integerToString(offsetX));
	image.setXmlParam("y", integerToString(offsetY));
	image.setXmlParam("w", integerToString(newW));
	image.setXmlParam("h", integerToString(newH));
	
}